import { useEffect, useRef } from 'react';
import type { Character } from '../types';

interface DuePost {
  characterId: string;
  /** RFC 3339 datetime — used as targetTime for post generation */
  targetTime: string;
  /** HH:MM — used to mark the slot as generated */
  timeStr: string;
}

interface Options {
  characters: Character[];
  socialMode: boolean;
  chatMode: boolean;
  /** Called after a post is successfully generated to refresh the feed. */
  onPostGenerated: (characterId: string) => void;
}

export function usePostGeneration({ characters, socialMode, onPostGenerated }: Options) {
  // Tracks "characterId|timeStr" combos processed this session to avoid duplicates on re-render
  const processedRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    if (!socialMode || characters.length === 0) return;

    const checkPosts = async () => {
      try {
        const { invoke } = await import('@tauri-apps/api/core');

        // get_due_posts: for each character, ensures today's schedule exists (LLM decides
        // posting times once per day), then returns all slots that are past-due.
        const duePosts = await invoke<DuePost[]>('get_due_posts');

        for (const due of duePosts) {
          const key = `${due.characterId}|${due.timeStr}`;
          if (processedRef.current.has(key)) continue;
          processedRef.current.add(key);

          try {
            console.log(`%c[PhoneClaw] Enqueuing post generation for character: ${due.characterId} (time: ${due.timeStr})`, 'color: #3b82f6;');
            const entryId = await invoke<string>('generate_character_post', {
              characterId: due.characterId,
              targetTime: due.targetTime,
            });
            // Mark as generated ONLY AFTER calling LLM and successfully placing into the queue
            if (entryId) {
              console.log(`%c[PhoneClaw] Successfully drafted post to queue! Queue Id: ${entryId}. Waiting for Ollama in background...`, 'color: #10b981; font-weight: bold;');
              await invoke('mark_post_generated', {
                characterId: due.characterId,
                timeStr: due.timeStr,
              }).catch(() => {});
            }
            onPostGenerated(due.characterId);
          } catch (e) {
            console.warn(`%c[PhoneClaw] Failed to enqueue post for ${due.characterId} at ${due.timeStr}:`, 'color: #f59e0b;', e);
            // Model unavailable or generation error — slot NOT marked, will retry on next polling
            processedRef.current.delete(key);
          }
        }

        // Resume/patrol queue in case things got stuck previously
        const resumedCount = await invoke<number>('resume_post_gen_queue').catch(() => 0);
        if (resumedCount > 0) {
            console.log(`%c[PhoneClaw] Background Patrol: Resumed and successfully completed ${resumedCount} interrupted reactions/posts.`, 'color: #8b5cf6;');
        }
      } catch {
        // Schedule system unavailable — skip silently
      }
    };

    // Run once immediately
    checkPosts();

    // Re-check every 5 minutes
    const interval = setInterval(checkPosts, 5 * 60 * 1000);
    return () => clearInterval(interval);
  }, [socialMode, characters]);
}
