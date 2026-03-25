import { useState } from 'react';
import { Heart, MessageCircle, Send, Trash2 } from 'lucide-react';
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
  onDm: () => void;
}

function PostCard({ post, character, characters, isLiked, onLike, onDelete, onDm }: PostCardProps) {
  const { comments, addComment } = usePostComments(post.id);
  const [commentText, setCommentText] = useState('');

  const submitComment = () => {
    const text = commentText.trim();
    if (!text) return;
    addComment('user', text);
    setCommentText('');
  };

  return (
    <div className="post-card">
      {/* Header */}
      <div className="post-card-header">
        <div className="post-card-avatar">
          {character?.icon
            ? <img src={character.icon} className="post-card-avatar-img" alt="" />
            : <div className="post-card-avatar-placeholder">
                {character?.name?.charAt(0).toUpperCase() ?? '?'}
              </div>
          }
        </div>
        <div className="post-card-meta">
          <span className="post-card-name">{character?.name ?? 'Unknown'}</span>
          <span className="post-card-time">{formatRelativeTime(post.createdAt)}</span>
        </div>
        <button
          className="post-card-delete"
          onClick={onDelete}
          aria-label="Delete post"
        >
          <Trash2 size={14} />
        </button>
      </div>

      {/* Image */}
      {post.image && (
        <img src={post.image} className="post-card-image" alt="" />
      )}

      {/* Text */}
      {post.text && (
        <p className="post-card-text">{post.text}</p>
      )}

      {/* Footer */}
      <div className="post-card-footer">
        <button className={`post-card-like-btn${isLiked ? ' post-card-like-btn--liked' : ''}`} onClick={onLike}>
          <Heart size={16} className="post-card-heart" fill={isLiked ? 'currentColor' : 'none'} />
          <span className="post-card-like-count">{post.likeCount}</span>
        </button>
        <button className="post-card-like-btn" onClick={onDm} aria-label="DM this character" title={`Message ${character?.name ?? 'character'}`}>
          <MessageCircle size={16} />
        </button>
        <span className="post-card-date">
          {new Date(post.createdAt).toLocaleString()}
        </span>
      </div>

      {/* Comments */}
      <div className="post-card-comments">
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

        {/* Comment input */}
        <div className="post-card-comment-input-row">
          <input
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
      </div>
    </div>
  );
}

interface PostFeedProps {
  posts: Post[];
  characters: Character[];
  likedPostIds: Set<string>;
  onLike: (id: string) => void;
  onDelete: (id: string) => void;
  onDmCharacter: (characterId: string, post: Post) => void;
}

export function PostFeed({ posts, characters, likedPostIds, onLike, onDelete, onDmCharacter }: PostFeedProps) {
  if (posts.length === 0) {
    return (
      <div className="post-feed-empty">
        <Heart size={40} className="post-feed-empty-icon" />
        <p>No posts yet.</p>
        <p className="post-feed-empty-sub">Characters will share posts here.</p>
      </div>
    );
  }

  return (
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
          onDm={() => onDmCharacter(post.characterId, post)}
        />
      ))}
    </div>
  );
}
