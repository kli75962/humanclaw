import { useEffect, useRef } from 'react';
import type { Character, Post } from '../types';

const LAST_GEN_KEY_PREFIX = 'phoneclaw_last_post_gen_';

/** Posts per day by frequency */
const POSTS_PER_DAY: Record<string, number> = {
  low: 1,
  medium: 2,
  high: 3,
};

function getPostsPerDay(_character: Character): number {
  return POSTS_PER_DAY['medium'] ?? 2;
}

/** Return the number of posts that should have been generated since lastGenAt. */
function calcMissedPosts(lastGenAt: string | null, postsPerDay: number): number {
  if (!lastGenAt) return 1; // first time — generate 1 to seed the feed
  const msSince = Date.now() - new Date(lastGenAt).getTime();
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

      for (const character of pending) {
        const storageKey = LAST_GEN_KEY_PREFIX + character.id;
        const lastGenAt = localStorage.getItem(storageKey);
        const postsPerDay = getPostsPerDay(character);
        const count = calcMissedPosts(lastGenAt, postsPerDay);

        for (let i = 0; i < count; i++) {
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

        localStorage.setItem(storageKey, new Date().toISOString());
      }
    })();
  }, [igMode, chatMode, characters]); // re-run when characters loads or changes
}
