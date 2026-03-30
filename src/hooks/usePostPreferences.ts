import { useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Post } from '../types';

export interface PostPreference {
  postId: string;
  characterId: string;
  postText: string;
  postContent: string;
  reason: string; // "auto-analyzed" or user-provided reason
  analysis: string; // LLM analysis of why user might not like it
  timestamp: string;
}

export function usePostPreferences() {
  const recordNotInterested = useCallback(async (
    post: Post,
    userReason?: string
  ): Promise<void> => {
    try {
      // Send to backend for analysis
      await invoke('record_post_preference', {
        postId: post.id,
        characterId: post.characterId,
        postText: post.text,
        postImage: post.image,
        userReason: userReason || '',
      });
    } catch (err) {
      console.error('Failed to record preference:', err);
    }
  }, []);

  const hidePost = useCallback(async (postId: string): Promise<void> => {
    try {
      await invoke('hide_post', { postId });
    } catch (err) {
      console.error('Failed to hide post:', err);
    }
  }, []);

  const removePost = useCallback(async (postId: string): Promise<void> => {
    try {
      await invoke('delete_post', { postId });
    } catch (err) {
      console.error('Failed to remove post:', err);
    }
  }, []);

  return {
    recordNotInterested,
    hidePost,
    removePost,
  };
}
