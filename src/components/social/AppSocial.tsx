import { useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { PostFeed } from '../social/PostFeed';
import type { Character, Post, Message } from '../../types';

interface AppSocialProps {
  posts: Post[];
  characters: Character[];
  likedPostIds: Set<string>;
  toggleLike: (id: string) => void;
  deletePost: (id: string) => void;
  addPost: (params: { characterId: string; text: string }) => Promise<Post>;
  refreshPosts: () => void;
  activeChatId: string | null;
  setInitMessages: (msgs: Message[]) => void;
}

export function AppSocial(props: AppSocialProps) {
  const {
    posts, characters, likedPostIds, toggleLike, deletePost, addPost, refreshPosts, activeChatId, setInitMessages
  } = props;

  const handleCreateUserPost = useCallback(async (text: string) => {
    const post = await addPost({ characterId: 'user', text });
    invoke<{ characterId: string; text: string }[]>('react_to_user_post', { postId: post.id })
      .then(async (dms) => {
        refreshPosts();
        for (const dm of dms) {
          const chatId = `char_${dm.characterId}`;
          const msgs = await invoke<{ role: string; content: string }[]>('load_chat_messages', { id: chatId }).catch(() => []);
          const updated = [...msgs, { role: 'assistant', content: dm.text }];
          await invoke('save_chat_messages', { id: chatId, messages: updated }).catch(() => {});
          // Refresh active chat if the user is already in it
          if (activeChatId === chatId) {
            setInitMessages(updated as Message[]);
          }
        }
      })
      .catch(() => {});
  }, [addPost, refreshPosts, activeChatId, setInitMessages]);

  return (
    <div className="app-content custom-scrollbar">
      <div className="app-posts-feed">
        <PostFeed
          posts={posts}
          characters={characters}
          likedPostIds={likedPostIds}
          onLike={toggleLike}
          onDelete={deletePost}
          onCreatePost={handleCreateUserPost}
        />
      </div>
    </div>
  );
}
