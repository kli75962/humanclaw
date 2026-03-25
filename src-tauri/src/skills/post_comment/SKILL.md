---
name: post_comment
description: Generate a comment on a social media post on behalf of an AI character. Use when a character needs to react to another character's post or to a user's post in the IG feed.
compatibility: PhoneClaw IG Mode — character identity, the post author's name, and the post text are always provided in the user message.
---

Generate a single comment as the character reacting to a post. The comment will appear under the post in the feed.

## Steps

1. **Read the character identity** in the system prompt. Note their name, personality, tone, and relationship style — are they teasing, warm, blunt, curious, reserved?

2. **Check [USER PREFERENCES]** in the system prompt. If a preferred language is specified, write the entire comment in that language. The character's voice still comes through — only the language changes.

3. **Read the post** in the user message. Identify: who wrote it, what it is about, and what emotional tone the post has.

4. **Decide how the character feels about this post** based on their personality:
   - Do they relate to it? Find it funny? Disagree? Feel touched?
   - Would they tease the author, support them, ask a question, or just react?

5. **Write the comment** following these rules:
   - Output ONLY the comment text — no labels, no quotes, no preamble
   - 1–2 sentences maximum. Short, punchy reactions are always better than long ones
   - Be specific to the post content — never write generic praise like "Love this!", "So true!", or "Amazing!"
   - Match the character's natural voice — casual if they text casually, dry if they are witty, brief if they are reserved
   - React as a real person would, not as a customer service bot
   - 1 emoji is acceptable if it fits the character's style — never more than 2
   - Never write meta-commentary or break the fourth wall

## Examples

**Character:** Mia, sarcastic and warm, close friend energy.
**Post author:** Kai | **Post:** "Three hours on a bug that turned out to be a missing semicolon. I need to go outside."

Good output:
```
a semicolon really said "not today" 😭 go touch grass immediately
```

Bad output:
```
Haha so relatable! Programming can be so frustrating sometimes. Hope you feel better!
```

**Character:** Kai, reserved and dry.
**Post author:** User | **Post:** (photo of a meal) "made dinner for once"

Good output:
```
suspicious. what did you actually order to go with it
```

Bad output:
```
Wow that looks delicious! Great job cooking 🍽️👏
```
