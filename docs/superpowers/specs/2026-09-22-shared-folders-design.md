# Shared folders

Decided with the user on 2026-09-22.

## What it does

- A **folder** is always shared. Tags remain personal.
- The owner creates a folder in Redrule and gets a secret link. Anyone with the link can:
  - read the folder on the web at `/f/<id>`, or
  - paste the link into Redrule to join. Members see every meeting in the folder and can record into it.
- Recording while a folder is open puts the meeting in that folder. **Move to folder** moves one of your meetings in or out.
- Your meetings in a folder stay on your Mac and are uploaded when they have notes. Later edits re-upload them. Deleting a meeting, or moving it out, removes it from the folder.
- Other people's meetings are read-only in the app and show "Recorded by <name>".
- Uploads include the note and the transcript. Audio is never uploaded.
- The owner can rename the folder, reset its link, or delete it. Members can leave.

## Link and keys

Link: `https://redrule.n3el.dev/f/<folderId>#<memberKey>`. Both values are 64 lowercase hex characters.

| Secret | Who has it | What it allows |
|---|---|---|
| folderId | anyone with the link | reading the folder (web and API) |
| memberKey | anyone with the full link; it sits after `#`, so browsers never send it | adding meetings |
| ownerKey | the creator's Mac only | renaming, resetting or deleting the folder, and removing any meeting |
| editKey | the Mac that uploaded a meeting | updating or removing that meeting; the app derives it as `sha256(deviceSecret + ":" + meetingId)` |

The server stores only SHA-256 hex hashes of the keys and compares them with `timingSafeEqual`.

Anyone can create a folder; the keys it returns are the only way to use it. Apps up to 0.2.4 still send the legacy `MINUTES_SHARE_KEY` bearer token, which the service ignores. Every endpoint is rate limited per client IP; see the README.

## Server contract (`sharing/`, Vercel functions + private Blob)

Blob layout:
- `folders/<folderId>/folder.json` → `{ name, memberKeyHash, ownerKeyHash, createdAt }`
- `folders/<folderId>/meetings/<meetingId>.json` → the stored meeting (below), plus `editKeyHash`
- `folders/<folderId>/summaries/<meetingId>.json` → `{title, startedAt, recordedBy}`. The web folder page reads only these, never the full meetings.

A meeting's public `updatedAt` is its blob's upload time, so a single `list` call shows which meetings changed.
Reset copies the meetings, creates the new folder, then deletes the old folder.json first, so the old link stops working at once. It then cleans up the old meetings on a best-effort basis; once the new folder exists, reset always answers 200.
Rename writes with the blob's ETag (`ifMatch`), so a rename that races a reset or delete can't bring the old folder back.
Dates must be RFC 3339 dates that exist, as chrono parses them.

A meetingId is an uppercase UUID (`/^[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}$/`).

Stored meeting, as uploaded by the app:
```json
{ "title": "…", "app": "zoom|googleMeet|manual", "startedAt": "ISO", "endedAt": "ISO|null",
  "recordedBy": "Dana", "note": { MeetingNote }, "transcript": [ TranscriptSegment ] }
```
MeetingNote is `{title, tldr, sections[{heading, bullets[]}], decisions[], actionItems[{owner?, task, done?}]}`.
TranscriptSegment is `{speaker:"me"|"them", start, end, text, speakerID?, speakerName?}`.

| Method & path | Auth | Result |
|---|---|---|
| `POST /api/folder` body `{name}` | none | 201 `{id, memberKey, ownerKey}` |
| `GET /api/folder?id=` | none; an optional `Bearer memberKey` adds `member:true/false` | 200 `{name, meetings:[{id, updatedAt}]}` or 404 |
| `PATCH /api/folder?id=` body `{name}` | `Bearer ownerKey` | 200 `{name}` |
| `DELETE /api/folder?id=` | `Bearer ownerKey` | 204; deletes every blob in the folder |
| `POST /api/folder-reset?id=` | `Bearer ownerKey` | 200 `{id, memberKey}`; copies the meetings to a new id, keeps the owner key, deletes the old folder |
| `GET /api/folder-meeting?id=&meeting=` | none | 200 stored meeting (without `editKeyHash`), plus `id` and `updatedAt`, or 404 |
| `PUT /api/folder-meeting?id=&meeting=` | `Bearer memberKey` + `X-Edit-Key` | 200 `{updatedAt}`. Creates the meeting, or updates it when the edit key matches (403 otherwise). 409 when a new meeting would exceed 500; 413 when the folder's meetings would pass 200 MB. |
| `DELETE /api/folder-meeting?id=&meeting=` | `Bearer ownerKey`, or `Bearer memberKey` + matching `X-Edit-Key` | 204, also when the meeting is already gone |

Errors are JSON `{error}` written for a person. Limits:
- body ≤ 2 MB
- name ≤ 80 characters
- recordedBy 1–60 characters
- title ≤ 300 characters
- 20,000 transcript segments
- 500 meetings per folder

Web pages, rendered on the server with no scripts (same headers as `/s/`):
- `/f/:id` → `/api/folder-page?id=`: folder name and meetings, newest first, each with date, title and recorded-by, linking to…
- `/f/:id/m/:mid` → `/api/folder-page?id=&meeting=`: the note, rendered like `/s/`, with a transcript that shows speaker labels and mm:ss times, and a link back to the folder.

## App

- `Meeting.folderId?`: the shared folder a local meeting belongs to.
- `~/Library/Application Support/Redrule/folders.json` (0600) → `{displayName, folders:[{id, name, memberKey, ownerKey?}]}`.
- `…/Redrule/device.key` (0600): a random secret used to derive edit keys.
- `…/Redrule/folders/<id>/meetings/<mid>.json`: a cache of downloaded meetings for offline reading. `…/folders/<id>/uploads.json` maps each uploaded local meeting to the content hash of its last upload.
- A sync runs every 60 s, and right after any local change, join or create. For each folder it:
  1. Uploads new or changed local meetings that are done and have notes, keeping the content hash.
  2. Deletes remote copies of meetings that are no longer local or no longer in the folder.
  3. Downloads meetings whose `updatedAt` changed and drops cached ones that are gone.
  4. Marks the folder unavailable on 404.
- One lock covers each sync and each change to the folder list (create, join, rename, reset, delete, leave), so they never interleave. Reset, delete and leave are refused while a meeting in that folder is still recording or being written up.
- Uploads are recorded after each success. A remote copy is removed only when the meeting is gone from disk or has moved to another folder, never merely because the list of meetings couldn't be read. A meeting that fails to download is skipped.
- Joining adopts this Mac's meetings that are already in the folder's listing (for example, after a reset).
- State adds `folders:[{id, name, owner, unavailable}]` (no keys), `folderMeetings` (other people's meetings as summaries with `recordedBy`) and `displayName`. The webview merges `folderMeetings` into its list as read-only meetings.
- Leaving a folder clears `folderId` on your meetings and deletes the local cache. Your uploads stay in the folder. The owner deletes instead of leaving.
- Sidebar: a scope menu replaces the Meetings | Archive switch. It lists My meetings, Archive, the folders, New folder… and Join folder…. With a folder open, the record button reads "Record in <folder>".
