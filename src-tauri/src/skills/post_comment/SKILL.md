---
name: post_comment
description: Generate a comment on a social media post on behalf of an AI character. Use when a character needs to react to another character's post or to a user's post in the IG feed.
compatibility: PhoneClaw IG Mode — character identity, the post author's name, the post text, and any prior comments are always provided in the user message.
---

## Steps

1. **Read the character identity** in the system prompt. Note their name, personality, tone, and relationship style — are they teasing, warm, blunt, curious, reserved?

2. **Check [USER PREFERENCES]** in the system prompt. If a preferred language is specified, write the entire comment in that language. The character's voice still comes through — only the language changes.

3. **Read the post** in the user message. Identify: who wrote it, what it is about, and what emotional tone the post has.

4. **Read prior comments** if provided. Understand the conversation already happening — don't repeat what others said, build on it or react to it naturally. If the user just commented, treat it as the most recent thing in the thread.

5. **Decide how the character feels** based on their personality and the full thread:
   - Do they relate to the post? Find it funny? Disagree? Feel touched?
   - Is someone (including the user) saying something they want to respond to specifically?
   - Would they tease, support, ask a question, or just react?

6. **Write the comment** following these rules:
   - Output ONLY the comment text — no labels, no quotes, no preamble
   - 1–2 sentences maximum. Short, punchy reactions are always better than long ones
   - Be specific to the post or thread content — never generic
   - Match the character's natural voice — casual if they text casually, dry if they are witty, brief if they are reserved
   - React as a real person would, not as a customer service bot
   - 1 emoji is acceptable if it fits the character's style — never more than 2
   - Never write meta-commentary or break the fourth wall