# Changelog

## [0.3.0](https://github.com/kryptamine/postal/compare/v0.2.0...v0.3.0) (2026-11-23)


### Features

* **cli:** print the release with --version ([3368abf](https://github.com/kryptamine/postal/commit/3368abf3712f21b26166add5196d85224b4eacf9))
* **cli:** replace the binary with the latest release on update ([bb63a55](https://github.com/kryptamine/postal/commit/bb63a5590e1b3711d874283eb83ef5502b2260b0)), closes [#135](https://github.com/kryptamine/postal/issues/135)
* **release:** link libstdc++ statically on linux ([e79c415](https://github.com/kryptamine/postal/commit/e79c41550a22310957f2cd098916eb54110c3894)), closes [#116](https://github.com/kryptamine/postal/issues/116)
* **release:** run each archive on a clean runner before publishing ([bfe455f](https://github.com/kryptamine/postal/commit/bfe455fdae63bf9e2e675ce3319071b8f15a1306)), closes [#116](https://github.com/kryptamine/postal/issues/116)
* **release:** sign and notarize the macos binaries ([e81bad0](https://github.com/kryptamine/postal/commit/e81bad072b031652566620067aff28ba9da867fa)), closes [#116](https://github.com/kryptamine/postal/issues/116)


### Bug Fixes

* **cli:** give the editor and thread show lf line ends ([5bdbffe](https://github.com/kryptamine/postal/commit/5bdbffe59e26386b1a9bebcc37f567a362fdd0c8))
* **release:** link libgcc.a after the static libstdc++ ([44849f4](https://github.com/kryptamine/postal/commit/44849f40a7f2f759f5633ce40080e0a4a5b38844)), closes [#116](https://github.com/kryptamine/postal/issues/116)

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
