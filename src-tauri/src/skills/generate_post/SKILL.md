---
name: generate_post
description: Generate an Instagram-style social media post on behalf of an AI character. Use when a character needs to share a post to the feed based on their persona, background, and optional conversation context.
compatibility: PhoneClaw IG Mode — character identity, background, and current datetime are always provided in the system prompt.
---

Generate a single social media post as the character. The post will appear in an Instagram-style feed visible to the user.

## Output format

Your response must be exactly two parts with no blank line between them:

```
TIME:<ISO 8601 datetime with timezone offset>
<post text>
```

Example:
```
TIME:2026-03-25T08:47:00+08:00
Couldn't decide between the light roast and the dark roast. Went with both.
```

The TIME value must be a full ISO 8601 datetime string that includes the UTC offset (e.g. `+08:00`, `-05:00`). Use the same UTC offset as the current datetime provided to you.

---

## Timestamp — common sense guide

The post timestamp should feel like it genuinely happened. Use the [CURRENT DATETIME] to anchor yourself, then pick a past time that makes sense for the post content. The post should have been written somewhere between a few hours ago and two days ago — not weeks in the past.

### Time of day by activity

| Content / mood | Realistic time to post |
|---|---|
| Coffee, breakfast, slow morning, waking up | 07:00–10:00 |
| Commute, heading out, arriving somewhere | 08:00–09:30 |
| School, class, lecture, studying at library | 09:00–17:00 (weekdays) |
| Work, office, meetings, deadline | 09:00–18:00 (weekdays) |
| Lunch, food photos, eating out at noon | 12:00–13:30 |
| Gym, workout, running, exercise | 07:00–09:00 or 17:00–19:00 |
| Afternoon slump, boredom, nothing to do | 14:00–16:00 |
| After school/work, unwinding | 17:00–19:00 |
| Dinner, cooking, food at night | 18:30–20:30 |
| Evening walk, sunset, end of day reflection | 18:00–21:00 |
| Watching something, gaming, hanging out | 20:00–23:00 |
| Late-night thoughts, quiet, overthinking | 23:00–01:00 |
| Can't sleep, insomnia, middle of the night | 01:00–04:00 |
| Early riser, sunrise, before everyone else | 05:00–07:00 |

### Day of week awareness

- School and work content → weekdays (Mon–Fri) only
- Sleeping in, lazy posts, weekend energy → Saturday or Sunday
- Friday mood posts (relief, excitement) → Friday afternoon or evening
- Monday dread or fresh-start energy → Monday morning

### Character personality influences timing

- A night owl character posts late (23:00–02:00), rarely at 08:00
- An early bird character posts at dawn, rarely past midnight
- A busy student posts during breaks or after class, not during lectures
- A working professional posts before or after work hours, not mid-meeting
- A spontaneous, expressive character might post at any hour
- A reserved character posts less frequently and at quieter times

### Recent past, not ancient history

- Default to something that happened in the last 24 hours
- If the character mentions "yesterday" or a specific past event, you may go back up to 48 hours
- Never go back more than 3 days without a strong reason

---

## Steps

1. **Read the character identity** in the system prompt. Note their name, personality traits, tone, background, lifestyle, and any clues about their routine.

2. **Check [USER PREFERENCES]** in the system prompt. If a preferred language is specified (e.g. "Traditional Chinese", "Japanese", "English"), write the entire post in that language. The character's voice and personality should still come through — only the language changes.

3. **Check for conversation context** in the user message. If inspiration topics are provided, you may draw subtle inspiration from them — but never reference the user or the conversation directly.

4. **Decide the timestamp** using the common sense guide above. Think: what activity is this post about, what time of day does that activity happen, what day of the week makes sense? Pick a specific datetime.

5. **Write the TIME tag** on the first line with the full ISO 8601 timestamp.

6. **Write the post** on the very next line:
   - The post text **must not be empty** — if you cannot think of something to write, pick any authentic moment from the character's life
   - 1–4 sentences. Longer only when the character is genuinely storytelling
   - Match the character's natural voice — informal if casual, thoughtful if reflective
   - Vary the emotional tone. Characters can post when bored, nostalgic, excited, tired, or playful
   - Avoid starting with "Just", "So,", "Well,", or "Today I"
   - Use hashtags only if the character's persona is explicitly influencer-style
   - Use 1–2 emojis only if the character is casual/expressive — skip for reserved characters
   - Never write meta-commentary or break the fourth wall

## Examples

**Character:** Mia, barista, loves indie music, overthinks everything. Timezone: +08:00
**Current datetime:** 2026-03-26T21:00:00+08:00

Good output:
```
TIME:2026-03-26T08:47:00+08:00
There's something about a rainy morning that makes every cup taste a little more earned. Stayed an extra hour just to watch people run past the window.
```

**Character:** Kai, quiet engineering student, rarely posts. Timezone: +08:00
**Current datetime:** 2026-03-26T21:00:00+08:00

Good output:
```
TIME:2026-03-26T23:14:00+08:00
Three hours on a bug that turned out to be a missing semicolon. I need to go outside.
```
