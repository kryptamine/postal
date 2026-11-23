# Changelog

## [0.2.0](https://github.com/kryptamine/postal/compare/v0.1.0...v0.2.0) (2026-11-23)


### ⚠ BREAKING CHANGES

* **nvim:** the plugin is no longer copied to ~/.local/share/postal/nvim; install it with a plugin manager instead.

### Features

* **landing:** copy the install command with one click ([ca2a0d4](https://github.com/kryptamine/postal/commit/ca2a0d4b299c511f5bdb29396056fff012660f5c))
* **nvim:** install the plugin through a plugin manager ([5832545](https://github.com/kryptamine/postal/commit/5832545cdc328d4de2e19de59076a1bec84fc627)), closes [#127](https://github.com/kryptamine/postal/issues/127)


### Bug Fixes

* **landing:** keep the space before a link that starts a line ([7ab4600](https://github.com/kryptamine/postal/commit/7ab4600127dbb4dd7824d1e8fc7079eca912d106))

## 0.1.0 (2026-11-23)


### Features

* **landing:** deploy the site to GitHub Pages ([f9cb273](https://github.com/kryptamine/postal/commit/f9cb273caf0b725255923ab83218c59ce983a710)), closes [#107](https://github.com/kryptamine/postal/issues/107)
* **release:** ship prebuilt binaries and an install script ([8fd2f32](https://github.com/kryptamine/postal/commit/8fd2f32c881ab5bf579eb701bbd9fbc1522eaa8e)), closes [#108](https://github.com/kryptamine/postal/issues/108)


### Bug Fixes

* **imap:** send without Nagle's delay ([ac37701](https://github.com/kryptamine/postal/commit/ac37701101eec422c65eaa7dcd36761c80dd39dc))
* satisfy the lints of clippy 1.100 ([0b221d0](https://github.com/kryptamine/postal/commit/0b221d08bae4b53333d6a78a313c6664afe48734))
* **ui:** keep a key typed while the session reloads ([69e90d8](https://github.com/kryptamine/postal/commit/69e90d848717b1bd4654d18eb6da8453663e1a07))


### Performance Improvements

* **ci:** skip the rust jobs when no rust input changed ([9053295](https://github.com/kryptamine/postal/commit/90532958d0d8015c8ec8d01b5cad0431671a29ac))
