---
name: phone-login
description: guide for logging in to Android apps, ONLY use this when you face the senario that describe in  phone-control
compatibility: PhoneClaw (Tauri v2 Android agent)
---

## Step 1 — Read the screen

Call get_screen to see what is on the screen. Identify:
- Whether there is an email/username input field and a password field
- Whether there are third-party login buttons (e.g. "Continue with Google", "Login with GitHub", "Sign in with Apple")
- The app package name from context

## Step 2 — Choose the login path

If the screen shows only email + password fields, go to Scenario A.
If the screen shows third-party login buttons (with or without email fields), go to Scenario B.

---

## Scenario A — Email and Password Fields

Tap the email field to focus it.
Call: fill_credential_field(app_package="<pkg>", field_type="email")

Possible results:
- "Field filled." — proceed to fill the password field
- status:forgot — tell the user you cannot proceed without their account info, then stop
- status:register — navigate to the registration page of this app
- voice_selection — interpret account_hint against available_accounts, then call commit_suggestion(index=<matched index>)

Tap the password field to focus it.
Call: fill_credential_field(app_package="<pkg>", field_type="password")

Possible results:
- "Field filled." — proceed to submit
- status:forgot — inform user, stop here, do not guess or type a password
- status:register — navigate to registration

Tap the login button or call press_key("enter").
fill_credential_field does NOT auto-submit. You must tap the button yourself.

---

## Scenario B — Third-party Login Options

Call get_screen to read all the login method buttons visible on screen.
List their labels exactly as they appear.
Call: show_login_method_picker(methods=["<label 1>", "<label 2>", ...])

The user will tap or speak their choice. You receive the selected method label.
Tap that button using the exact label returned: tap("<selected_method>")

After tapping:
- If the provider shows its own account picker — wait for it and proceed as needed
- If an email/password form appears — use Scenario A

---

## Rules

Never call type_text for a password field unless the user explicitly tells you to.
fill_credential_field handles stored credentials, autofill, and all overlay interactions internally — no other tool is needed for credential filling.
You must always tap the login button after fields are filled — filling is not submitting.
For voice_selection results: interpret the spoken hint (e.g. "first one", "the gmail one") against the available_accounts list, then call commit_suggestion with the matching index.
