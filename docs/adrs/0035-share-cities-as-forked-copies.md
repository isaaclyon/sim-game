# 0035. Share cities as files and links that open as independent copies

- Status: Accepted; implemented
- Date: 2026-11-03
- Extends: [0033](0033-single-city-state-saves.md)

## Context

Players want to send a city to a friend who keeps building it. There is no server, account or
cloud storage, and the game is a static site. ADR 0033 already defines a validated, versioned
full-state save and one local slot per browser.

## Decision

- The shared format is the ADR 0033 save bytes, unchanged. JavaScript never decodes them; the
  sim validates them before anything is replaced.
- **File:** Download writes the save bytes as a `.city` file. Open file reads one back.
- **Link:** the save bytes, compressed with raw deflate and base64url-encoded, follow `#city=`
  in the URL fragment. Browsers do not send fragments to servers, so nothing is uploaded. Links
  longer than 1,000,000 characters are refused with advice to use the file instead. The link
  is shown in a field as well as copied, so a refused clipboard does not block sharing.
- **Opening forks the city.** An opened city replaces the current city and the local save
  slot as an independent copy, opened paused. Nothing links back to the sender; each person's
  later edits stay in their own browser. A reload resumes the local copy: the page removes
  the fragment once it has read it.
- Opening asks for confirmation whenever it would discard a city: always for files and for
  links pasted into a running game, and for links opened at startup unless the browser had no
  saved city and no storage error. Cancel changes nothing. A
  damaged link or a file the sim rejects leaves the current city and its save untouched.
- Replacement follows ADR 0033's confirmed New city rules: it is ordered after pending saves,
  may replace a save written by another tab, and reports failure honestly. Opened cities keep
  no command log (ADR 0034).

## Consequences

Sharing needs no backend and works from any static host. Shared cities only open on builds
that read the same save version, and a city carries no history of who built what. Large cities
are file-only, and links of a few hundred kilobytes may still be cut by chat apps. Sharing a
link exposes the whole city to anyone who has it. A server-side short link, thumbnails or
merging changes back would need a new decision.
