## Purpose

Give meetings a reliable subject and scheduled range from the operating-system calendar, and
corroborate them against observed attendance, so that reports distinguish a meeting that happened
from one that was merely scheduled.

## ADDED Requirements

### Requirement: Calendar access is consented, read-only, and optional
The system SHALL request operating-system calendar permission as part of the existing setup flow and
SHALL read calendar data only after permission is granted. The system MUST NOT create, modify, or
delete calendar entries, and every other capability MUST remain functional when access is declined.

#### Scenario: User declines calendar permission
- **WHEN** calendar permission is denied or later revoked
- **THEN** meeting entities are not produced, the setup surface explains the reduced coverage, and
  capture, segmentation, digests, and export continue for all other sources

#### Scenario: Permission granted mid-session
- **WHEN** the user grants calendar permission after collection has started
- **THEN** meeting entities become available for subsequent ranges without restarting collection

### Requirement: Calendar evidence is limited to meeting metadata
The system SHALL read subject, start and end times, all-day status, organizer, calendar name, and
attendance response. The system MUST NOT read attachments, meeting notes, transcripts, attendee
contact details beyond organizer identity, or entries from calendars the user has excluded.

#### Scenario: Calendar is excluded
- **WHEN** the user excludes a calendar
- **THEN** no entry from that calendar reaches storage, including entries that overlap observed
  conferencing activity

#### Scenario: Entry carries a private classification
- **WHEN** a calendar entry is marked private by the calendar store
- **THEN** it is represented as a busy period without its subject unless the user opts in to include
  private entries

### Requirement: Scheduled meetings are corroborated against observed attendance
The system SHALL combine a calendar entry with observed conferencing-application activity over the
same period to derive attendance and actual duration, and SHALL mark a meeting as unattended when no
corroborating activity is observed.

#### Scenario: Meeting is cancelled but remains on the calendar
- **WHEN** a calendar entry exists and no conferencing activity is observed during its range
- **THEN** the meeting is marked unattended and is excluded from report drafts by default

#### Scenario: Meeting runs past its scheduled end
- **WHEN** conferencing activity continues beyond the scheduled end time
- **THEN** observed duration is reported alongside scheduled duration rather than replacing it

#### Scenario: Conferencing activity has no matching calendar entry
- **WHEN** conferencing activity is observed with no corresponding calendar entry
- **THEN** a meeting is still represented with its observed range and without a fabricated subject
