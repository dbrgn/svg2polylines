# Releasing

Run linting:

    $ cargo clean && cargo clippy --all-targets --all-features

Set variables:

    $ export VERSION=X.Y.Z
    $ export GPG_KEY=EA456E8BAF0109429583EED83578F667F2F3A5FA

Update version numbers:

    $ cd svg2polylines
    $ vim Cargo.toml
    $ cargo update
    $ cd -

Update changelog:

    $ vim svg2polylines/CHANGELOG.md

Commit & tag:

    $ git commit -S${GPG_KEY} -m "Release v${VERSION}"
    $ git tag -s -u ${GPG_KEY} v${VERSION} -m "Version ${VERSION}"

Publish:

    $ cargo publish
    $ git push && git push --tags
