# Signing Template

`entrance.key.pub` is retained as reusable public signing metadata.

The matching private key is a local fixture and must stay outside Git tracking
under `entrance-auto/fixtures/private/` or another operator-controlled secret
location. Rotate it if it was ever used as a real release signing key.
