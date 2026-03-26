import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Post, PostComment } from '../types';

const LIKED_POSTS_KEY = 'phoneclaw_liked_posts';

function loadLikedIds(): Set<string> {
  try {
    return new Set(JSON.parse(localStorage.getItem(LIKED_POSTS_KEY) ?? '[]'));
  } catch {
    return new Set();
  }
}

export function usePosts() {
  const [posts, setPosts] = useState<Post[]>([]);
  const [likedPostIds, setLikedPostIds] = useState<Set<string>>(loadLikedIds);

  const refresh = useCallback(() => {
    invoke<Post[]>('list_posts').then(setPosts).catch(() => {});
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const addPost = useCallback(async (data: Omit<Post, 'id' | 'createdAt' | 'likeCount'>): Promise<Post> => {
    const newPost: Post = {
      ...data,
      id: crypto.randomUUID(),
      createdAt: new Date().toISOString(),
      likeCount: 0,
    };
    await invoke('save_post', { post: newPost });
    setPosts((prev) => [newPost, ...prev]);
    return newPost;
  }, []);

  const deletePost = useCallback(async (id: string) => {
    await invoke('delete_post', { id }).catch(() => {});
    setPosts((prev) => prev.filter((p) => p.id !== id));
  }, []);

  const toggleLike = useCallback(async (id: string) => {
    const isLiked = likedPostIds.has(id);
    const newCount = await invoke<number>(isLiked ? 'unlike_post' : 'like_post', { id }).catch(() => null);
    if (newCount !== null) {
      setPosts((prev) => prev.map((p) => p.id === id ? { ...p, likeCount: newCount } : p));
      setLikedPostIds((prev) => {
        const next = new Set(prev);
        if (isLiked) next.delete(id); else next.add(id);
        localStorage.setItem(LIKED_POSTS_KEY, JSON.stringify([...next]));
        return next;
      });
    }
  }, [likedPostIds]);

  /** Generate an AI post for a character and add it to the feed. */
  const generatePost = useCallback(async (characterId: string, context?: string): Promise<Post | null> => {
    try {
      const post = await invoke<Post>('generate_character_post', { characterId, context });
      setPosts((prev) => [post, ...prev]);
      return post;
    } catch {
      return null;
    }
  }, []);

  return { posts, likedPostIds, addPost, deletePost, toggleLike, refresh, generatePost };
}

export function usePostComments(postId: string) {
  const [comments, setComments] = useState<PostComment[]>([]);

  const refresh = useCallback(() => {
    invoke<PostComment[]>('list_comments', { postId }).then(setComments).catch(() => {});
  }, [postId]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const addComment = useCallback(async (authorId: string, text: string) => {
    const comment: PostComment = {
      id: crypto.randomUUID(),
      postId,
      authorId,
      text,
      createdAt: new Date().toISOString(),
    };
    await invoke('add_comment', { comment });
    setComments((prev) => [...prev, comment]);

    if (authorId === 'user') {
      // Fire-and-forget: let characters react based on personality, then refresh
      invoke('react_to_user_comment', { postId })
        .then(() => invoke<PostComment[]>('list_comments', { postId }))
        .then((updated) => setComments(updated as PostComment[]))
        .catch(() => {});
    }
  }, [postId]);

  return { comments, addComment, refresh };
}
