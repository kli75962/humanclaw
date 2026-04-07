import { useEffect, useRef } from 'react';
import type { Character, Post } from '../types';

interface DuePost {
  character_id: string;
  /** RFC 3339 datetime — used as targetTime for post generation */
  target_time: string;
  /** HH:MM — used to mark the slot as generated */
  time_str: string;
}

interface Options {
  characters: Character[];
  igMode: boolean;
  chatMode: boolean;
  /** Called after a post is successfully generated to refresh the feed. */
  onPostGenerated: (characterId: string) => void;
}

export function usePostGeneration({ characters, igMode, onPostGenerated }: Options) {
  // Tracks "characterId|timeStr" combos processed this session to avoid duplicates on re-render
  const processedRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    if (!igMode || characters.length === 0) return;

    (async () => {
      const { invoke } = await import('@tauri-apps/api/core');

      try {
        // get_due_posts: for each character, ensures today's schedule exists (LLM decides
        // posting times once per day), then returns all slots that are past-due.
        const duePosts = await invoke<DuePost[]>('get_due_posts');

        for (const due of duePosts) {
          const key = `${due.character_id}|${due.time_str}`;
          if (processedRef.current.has(key)) continue;
          processedRef.current.add(key);

          // Mark as generated BEFORE calling LLM to prevent duplicates on crash/re-open
          await invoke('mark_post_generated', {
            characterId: due.character_id,
            timeStr: due.time_str,
          }).catch(() => {});

          try {
            const post = await invoke<Post>('generate_character_post', {
              characterId: due.character_id,
              context: null,
              targetTime: due.target_time,
            });
            onPostGenerated(due.character_id);
            invoke('trigger_character_reactions', { postId: post.id }).catch(() => {});
          } catch {
            // Model unavailable or generation error — slot stays marked, won't retry
          }
        }
      } catch {
        // Schedule system unavailable — skip silently
      }
    })();
  }, [igMode, characters]);
}
