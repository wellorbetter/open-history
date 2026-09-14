# Privacy model

- Collection is off until consent and operating-system permission are both present.
- The capture boundary accepts semantic accessibility events only. Screenshots, audio, and raw
  keystroke sequences are out of scope and must not be linked.
- Application, window, website, and private-browser exclusions run before persistence.
- Raw events expire after 48 hours by default. Derived summaries remain until deletion.
- SQLCipher protects the database; its random key and local-client credentials live in Keychain or
  Windows Credential Manager.
- Activity capture, storage, indexing, and summarization remain on device. The product has no
  cloud-processing, cloud-sync, telemetry, or remote-history endpoint.
- On-device model enrichment and local agent access are separate opt-in capabilities.
- Captured strings are untrusted data. They never become system instructions or executable actions.
