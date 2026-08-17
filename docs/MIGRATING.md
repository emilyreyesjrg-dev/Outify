# Migrating from versions < 1.8.0
Release 1.8.0 released with new Spotify client credentials, which are critical for Outify to work as expected.
Old credentials are being revoked September 1st 2026 and are no longer being supported after that date.

To migrate, simply update the app, open Accounts settings and logout and login in both Playback and Account login.
If the app crashes before you can logout - clear cache and data and try again.

If the app crashes with message "No Spotify credentials were supplied during build", verify your build is correct (specifically checkout [Requirements](https://github.com/iTomKo/Outify/blob/master/docs/CONTRIBUTING.md))
- if it's official GitHub Release, please notify us in issues.
