import { useEffect, useRef } from 'react';
import type { Character, Post } from '../types';

interface PostGenEntry {
  id: string;
  character_id: string;
  post_id: string | null;
  generated_text: string;
  generated_timestamp: string;
  state: 'post_generating' | 'post_created' | 'reactions_in_progress' | 'completed' | 'failed';
  comments: unknown[];
  likes: unknown[];
  error: string | null;
  created_at: number; // Unix timestamp (seconds)
  updated_at: number;
}

/** Posts per day by frequency */
const POSTS_PER_DAY: Record<string, number> = {
  low: 1,
  medium: 2,
  high: 3,
};

function getPostsPerDay(_character: Character): number {
  return POSTS_PER_DAY['medium'] ?? 2;
}

/**
 * Calculate if a character should post right now based on their persona.
 * This adds personality-based decision-making to post generation.
 *
 * Examples:
 * - Introverts post less frequently (might skip some intervals)
 * - Extroverts post more eagerly
 * - Night owls post at unusual hours
 */
function shouldCharacterPost(character: Character, seed: string): boolean {
  const lower = character.persona.toLowerCase();

  // Extroverts: always post (100% chance)
  if (lower.includes('extrovert') || lower.includes('outgoing') || lower.includes('social')) {
    return true;
  }

  // Introverts: selective posting (50% chance)
  if (lower.includes('introvert') || lower.includes('quiet') || lower.includes('reserved')) {
    return simpleHash(seed) % 100 < 50;
  }

  // Default: always post
  return true;
}

/** Simple hash function for deterministic randomness */
function simpleHash(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) {
    h = ((h << 5) - h) + s.charCodeAt(i);
    h = h & h; // Convert to 32bit integer
  }
  return Math.abs(h);
}

interface RecentActivity {
  /** How many posts made in the last 24 hours */
  recentPostCount: number;
  /** Days since the character's last post */
  daysSinceLastPost: number;
  /** Total posts from this character */
  totalPosts: number;
  /** Average posts per week */
  avgPostsPerWeek: number;
}

/**
 * Advanced decision: should character post based on activity patterns?
 * Factors:
 * - Base personality (introvert/extrovert)
 * - Recent activity (posted too much lately?)
 * - Long time without posting (eager to share?)
 * - Time of day awareness (night owl vs early bird)
 * - Overall engagement level
 */
function shouldCharacterPostAdvanced(
  character: Character,
  seed: string,
  activity: RecentActivity
): boolean {
  const persona = character.persona.toLowerCase();

  // Start with base personality tendency
  let probability = 50; // neutral default

  // 1️⃣ Base personality → extroverts more likely to post
  if (persona.includes('extrovert') || persona.includes('outgoing') || persona.includes('social')) {
    probability = 80;
  } else if (
    persona.includes('introvert') ||
    persona.includes('quiet') ||
    persona.includes('reserved')
  ) {
    probability = 40;
  }

  // 2️⃣ Recent activity adjustment
  // If posted a lot in last 24 hours, reduce motivation
  if (activity.recentPostCount >= 3) {
    probability *= 0.6; // 40% reduction
  } else if (activity.recentPostCount >= 2) {
    probability *= 0.8; // 20% reduction
  }

  // 3️⃣ Long time without posting → eager to share again
  if (activity.daysSinceLastPost > 7) {
    probability *= 1.4; // 40% boost
  } else if (activity.daysSinceLastPost > 3) {
    probability *= 1.2; // 20% boost
  }

  // 4️⃣ Engagement level (total posts reflect engagement)
  // Characters with very few total posts are shyer
  if (activity.totalPosts < 3) {
    probability *= 0.7; // Less confident
  } else if (activity.totalPosts > 20) {
    // Very active characters might post less frequently (burnout/life balance)
    probability *= 0.9;
  }

  // 5️⃣ Time-based personality adjustments
  const hour = new Date().getHours();

  // Night owls: less likely to post during day
  if (
    persona.includes('night owl') ||
    persona.includes('nocturnal') ||
    persona.includes('late night')
  ) {
    if (hour >= 8 && hour <= 18) {
      probability *= 0.7; // Less active during day
    }
  }

  // Early bird: less likely to post at night
  if (
    persona.includes('early bird') ||
    persona.includes('morning person') ||
    persona.includes('sunrise')
  ) {
    if (hour >= 20 || hour <= 6) {
      probability *= 0.6; // Less active at night
    }
  }

  // 6️⃣ Burnout prevention: very active characters post less
  if (activity.avgPostsPerWeek > 10) {
    probability *= 0.8; // Reduced to prevent burnout
  }

  // Clamp probability to [0, 100]
  probability = Math.max(0, Math.min(100, probability));

  // Final decision based on probability
  return simpleHash(seed) % 100 < probability;
}

/** Get the last generation timestamp for a character from the queue. */
function getLastGenTimestamp(queue: PostGenEntry[], characterId: string): number | null {
  const entries = queue
    .filter((e) => e.character_id === characterId && e.state === 'completed')
    .sort((a, b) => b.created_at - a.created_at);

  return entries.length > 0 ? entries[0].created_at * 1000 : null; // Convert to milliseconds
}

/** Return the number of posts that should have been generated since lastGenMs. */
function calcMissedPosts(lastGenMs: number | null, postsPerDay: number): number {
  if (!lastGenMs) return 1; // first time — generate 1 to seed the feed
  const msSince = Date.now() - lastGenMs;
  const hoursSince = msSince / 3_600_000;
  const intervalHours = 24 / postsPerDay;
  const missed = Math.floor(hoursSince / intervalHours);
  return Math.min(missed, 3); // cap at 3 catch-up posts per session
}

interface Options {
  characters: Character[];
  igMode: boolean;
  chatMode: boolean;
  /** Called after a post is successfully generated to refresh the feed. */
  onPostGenerated: (characterId: string) => void;
}

export function usePostGeneration({ characters, igMode, chatMode, onPostGenerated }: Options) {
  // Track which character IDs we've already kicked off generation for this session
  const generatedRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    if (!igMode || !chatMode || characters.length === 0) return;

    // Only process characters we haven't handled yet this session
    const pending = characters.filter((c) => !generatedRef.current.has(c.id));
    if (pending.length === 0) return;

    pending.forEach((c) => generatedRef.current.add(c.id));

    (async () => {
      const { invoke } = await import('@tauri-apps/api/core');

      try {
        // Load the post generation queue to check last generation times and activity
        const queue = await invoke<PostGenEntry[]>('get_post_gen_pending');
        const allQueue = await invoke<PostGenEntry[]>('get_post_gen_queue'); // All entries (including completed)

        for (const character of pending) {
          const lastGenMs = getLastGenTimestamp(queue, character.id);
          const postsPerDay = getPostsPerDay(character);
          const count = calcMissedPosts(lastGenMs, postsPerDay);

          // Calculate recent activity for this character
          const now = Date.now();
          const dayMs = 24 * 60 * 60 * 1000;

          const charEntries = allQueue.filter((e) => e.character_id === character.id);
          const completedEntries = charEntries.filter((e) => e.state === 'completed');

          // Recent posts (last 24 hours)
          const recentPostCount = completedEntries.filter(
            (e) => now - e.created_at * 1000 < dayMs
          ).length;

          // Days since last post
          const lastPost = completedEntries[completedEntries.length - 1];
          const daysSinceLastPost = lastPost
            ? Math.floor((now - lastPost.created_at * 1000) / dayMs)
            : 999;

          // Total posts
          const totalPosts = completedEntries.length;

          // Average posts per week (if data available)
          const avgPostsPerWeek =
            completedEntries.length > 0
              ? (completedEntries.length / Math.max(daysSinceLastPost || 1, 1)) * 7
              : 0;

          const activity: RecentActivity = {
            recentPostCount,
            daysSinceLastPost,
            totalPosts,
            avgPostsPerWeek,
          };

          for (let i = 0; i < count; i++) {
            // Advanced character decision: should they post based on activity & personality?
            const seed = `${character.id}_post_${i}_${new Date().toDateString()}`;
            if (!shouldCharacterPostAdvanced(character, seed, activity)) {
              // This character decides not to post this time
              continue;
            }

            try {
              const post = await invoke<Post>('generate_character_post', {
                characterId: character.id,
                context: null,
              });
              onPostGenerated(character.id);
              invoke('trigger_character_reactions', { postId: post.id }).catch(() => {});
            } catch {
              // Model may be unavailable — skip silently
            }
          }
        }
      } catch {
        // Queue system unavailable, fall back to simple decision
        for (const character of pending) {
          const seed = `${character.id}_fallback_${new Date().toDateString()}`;
          if (!shouldCharacterPost(character, seed)) {
            continue;
          }

          try {
            const post = await invoke<Post>('generate_character_post', {
              characterId: character.id,
              context: null,
            });
            onPostGenerated(character.id);
            invoke('trigger_character_reactions', { postId: post.id }).catch(() => {});
          } catch {
            // Model may be unavailable — skip silently
          }
        }
      }
    })();
  }, [igMode, chatMode, characters]); // re-run when characters loads or changes
}
