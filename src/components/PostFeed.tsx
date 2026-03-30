import { useEffect, useRef, useState } from 'react';
import { Heart, MessageCircle, PenSquare, Send, Forward, Trash2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import type { Character, Post } from '../types';
import { usePostComments } from '../hooks/usePosts';
import '../style/PostFeed.css';

function formatRelativeTime(isoString: string): string {
  const diff = Date.now() - new Date(isoString).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins}m`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days}d`;
  return new Date(isoString).toLocaleDateString();
}

interface PostCardProps {
  post: Post;
  character: Character | undefined;
  characters: Character[];
  isLiked: boolean;
  onLike: () => void;
  onDelete: () => void;
}

function PostCard({ post, character, characters, isLiked, onLike, onDelete }: PostCardProps) {
  const isUserPost = post.characterId === 'user';
  const displayName = isUserPost ? 'You' : (character?.name ?? 'Unknown');
  const { comments, addComment } = usePostComments(post.id);
  const [commentText, setCommentText] = useState('');
  const [showCommentInput, setShowCommentInput] = useState(false);
  const [showShare, setShowShare] = useState(false);
  const [shareText, setShareText] = useState('');
  const [selectedChars, setSelectedChars] = useState<Set<string>>(new Set());
  const [sharing, setSharing] = useState(false);
  const commentInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (showCommentInput) commentInputRef.current?.focus();
  }, [showCommentInput]);

  const submitComment = () => {
    const text = commentText.trim();
    if (!text) return;
    addComment('user', text);
    setCommentText('');
  };

  const toggleChar = (id: string) => {
    setSelectedChars((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };

  const handleShare = async () => {
    if (selectedChars.size === 0 || sharing) return;
    setSharing(true);
    const authorLabel = isUserPost ? 'You' : (character?.name ?? 'Unknown');
    const postRef = `${authorLabel} posted: "${post.text}"`;
    const message = shareText.trim()
      ? `${postRef}\n\n${shareText.trim()}`
      : postRef;
    for (const charId of selectedChars) {
      const chatId = `char_${charId}`;
      const msgs = await invoke<{ role: string; content: string }[]>(
        'load_chat_messages', { id: chatId }
      ).catch(() => []);
      await invoke('save_chat_messages', {
        id: chatId,
        messages: [...msgs, { role: 'user', content: message }],
      }).catch(() => {});
    }
    setSharing(false);
    setShowShare(false);
    setShareText('');
    setSelectedChars(new Set());
  };

  return (
    <div className="post-card">
      {/* Header */}
      <div className="post-card-header">
        <div className="post-card-avatar">
          {isUserPost ? (
            <div className="post-card-avatar-placeholder">U</div>
          ) : character?.icon ? (
            <img src={character.icon} className="post-card-avatar-img" alt="" />
          ) : (
            <div className="post-card-avatar-placeholder">
              {character?.name?.charAt(0).toUpperCase() ?? '?'}
            </div>
          )}
        </div>
        <div className="post-card-meta">
          <span className="post-card-name">{displayName}</span>
          <span className="post-card-time">{formatRelativeTime(post.createdAt)}</span>
        </div>
        <button className="post-card-delete" onClick={onDelete} aria-label="Delete post">
          <Trash2 size={14} />
        </button>
      </div>

      {/* Image */}
      {post.image && <img src={post.image} className="post-card-image" alt="" />}

      {/* Text */}
      {post.text && <p className="post-card-text">{post.text}</p>}

      {/* Footer */}
      <div className="post-card-footer">
        <div className="post-card-actions">
          <button
            className={`post-card-like-btn${isLiked ? ' post-card-like-btn--liked' : ''}`}
            onClick={onLike}
          >
            <Heart size={16} className="post-card-heart" fill={isLiked ? 'currentColor' : 'none'} />
            <span className="post-card-like-count">{post.likeCount}</span>
          </button>
          <button
            className={`post-card-like-btn${showCommentInput ? ' post-card-like-btn--active' : ''}`}
            onClick={() => setShowCommentInput((v) => !v)}
            aria-label="Comment"
          >
            <MessageCircle size={16} />
            {comments.length > 0 && (
              <span className="post-card-like-count">{comments.length}</span>
            )}
          </button>
          <button
            className="post-card-like-btn"
            onClick={() => setShowShare(true)}
            aria-label="Share"
          >
            <Forward size={16} />
          </button>
        </div>
        <span className="post-card-date">{new Date(post.createdAt).toLocaleString()}</span>
      </div>

      {/* Comments */}
      {(comments.length > 0 || showCommentInput) && (
        <div className="post-card-comments">
          {showCommentInput && (
            <div className="post-card-comment-input-row">
              <input
                ref={commentInputRef}
                className="post-card-comment-input"
                placeholder="Add a comment…"
                value={commentText}
                onChange={(e) => setCommentText(e.target.value)}
                onKeyDown={(e) => { if (e.key === 'Enter') submitComment(); }}
              />
              <button
                className="post-card-comment-send"
                onClick={submitComment}
                disabled={!commentText.trim()}
                aria-label="Send comment"
              >
                <Send size={14} />
              </button>
            </div>
          )}

          {comments.map((c) => {
            const author = c.authorId === 'user'
              ? null
              : characters.find((ch) => ch.id === c.authorId);
            const label = c.authorId === 'user' ? 'You' : (author?.name ?? 'Unknown');
            return (
              <div key={c.id} className="post-card-comment">
                {c.authorId === 'user' ? (
                  <div className="post-card-comment-avatar-placeholder">U</div>
                ) : author?.icon ? (
                  <img src={author.icon} className="post-card-comment-avatar" alt="" />
                ) : (
                  <div className="post-card-comment-avatar-placeholder">
                    {label.charAt(0).toUpperCase()}
                  </div>
                )}
                <div className="post-card-comment-body">
                  <span className="post-card-comment-author">{label}</span>
                  <span className="post-card-comment-text">{c.text}</span>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Share modal */}
      {showShare && (
        <>
          <div className="post-share-backdrop" onClick={() => setShowShare(false)} />
          <div className="post-share-modal">
            <div className="post-share-title">Share post</div>
            <div className="post-share-preview">{post.text}</div>
            <textarea
              className="post-share-textarea"
              placeholder="Add a message…"
              value={shareText}
              onChange={(e) => setShareText(e.target.value)}
            />
            <div className="post-share-char-list">
              {characters.map((c) => (
                <div
                  key={c.id}
                  className={`post-share-char-item${selectedChars.has(c.id) ? ' post-share-char-item--selected' : ''}`}
                  onClick={() => toggleChar(c.id)}
                >
                  {c.icon ? (
                    <img src={c.icon} className="post-share-char-avatar" alt="" />
                  ) : (
                    <div className="post-share-char-avatar-placeholder">
                      {c.name.charAt(0).toUpperCase()}
                    </div>
                  )}
                  <span className="post-share-char-name">{c.name}</span>
                  <div className={`post-share-char-check${selectedChars.has(c.id) ? ' post-share-char-check--on' : ''}`} />
                </div>
              ))}
            </div>
            <div className="post-share-footer">
              <button
                className="post-share-send-btn"
                onClick={handleShare}
                disabled={selectedChars.size === 0 || sharing}
              >
                {sharing ? 'Sending…' : 'Send'}
              </button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}

interface PostFeedProps {
  posts: Post[];
  characters: Character[];
  likedPostIds: Set<string>;
  onLike: (id: string) => void;
  onDelete: (id: string) => void;
  onCreatePost: (text: string) => Promise<void>;
}

export function PostFeed({ posts, characters, likedPostIds, onLike, onDelete, onCreatePost }: PostFeedProps) {
  const [showCompose, setShowCompose] = useState(false);
  const [postText, setPostText] = useState('');
  const [posting, setPosting] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  async function handlePost() {
    const text = postText.trim();
    if (!text || posting) return;
    setPosting(true);
    try {
      await onCreatePost(text);
      setPostText('');
      setShowCompose(false);
    } finally {
      setPosting(false);
    }
  }

  function openCompose() {
    setShowCompose(true);
    setTimeout(() => textareaRef.current?.focus(), 0);
  }

  return (
    <>
      {posts.length === 0 ? (
        <div className="post-feed-empty">
          <Heart size={40} className="post-feed-empty-icon" />
          <p>No posts yet.</p>
          <p className="post-feed-empty-sub">Characters will share posts here.</p>
        </div>
      ) : (
        <div className="post-feed">
          {posts.map((post) => (
            <PostCard
              key={post.id}
              post={post}
              character={characters.find((c) => c.id === post.characterId)}
              characters={characters}
              isLiked={likedPostIds.has(post.id)}
              onLike={() => onLike(post.id)}
              onDelete={() => onDelete(post.id)}
            />
          ))}
        </div>
      )}

      {/* Compose popup — anchored above FAB */}
      {showCompose && (
        <>
          <div className="post-compose-backdrop" onClick={() => setShowCompose(false)} />
          <div className="post-compose">
            <textarea
              ref={textareaRef}
              className="post-compose-textarea"
              placeholder="What's on your mind?"
              value={postText}
              onChange={(e) => setPostText(e.target.value)}
              onKeyDown={(e) => { if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) handlePost(); }}
            />
            <div className="post-compose-footer">
              <button
                className="post-compose-post-btn"
                onClick={handlePost}
                disabled={!postText.trim() || posting}
              >
                {posting ? 'Posting…' : 'Post'}
              </button>
            </div>
          </div>
        </>
      )}

      {/* FAB */}
      <button
        className={`post-fab${showCompose ? ' post-fab--active' : ''}`}
        onClick={() => showCompose ? setShowCompose(false) : openCompose()}
        aria-label="Create post"
      >
        <PenSquare size={20} />
      </button>
    </>
  );
}
