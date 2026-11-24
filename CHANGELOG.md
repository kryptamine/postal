# Changelog

## [0.4.0](https://github.com/kryptamine/postal/compare/v0.3.0...v0.4.0) (2026-11-24)


### Features

* **cli:** draw the solid red stamp in the help lockup ([30ff85c](https://github.com/kryptamine/postal/commit/30ff85c2130392b68cffd19a52294a4195f94ca5))
* **cli:** open the top-level help with the terminal lockup ([5993db9](https://github.com/kryptamine/postal/commit/5993db99aa1848ee14340715e5e2fbf96e21a496))
* **cli:** sign a local install with the developer id when present ([0559db3](https://github.com/kryptamine/postal/commit/0559db310ba3bd1096a66e8e6aaefe748b87c29e))
* **landing:** put the install command in the hero and name the problem ([6911140](https://github.com/kryptamine/postal/commit/691114064ac983b33dda6f7fe55393a9c58c743f))
* **landing:** show Search operators, Settings, Picks, mcp and update ([cddf93c](https://github.com/kryptamine/postal/commit/cddf93caaef30d8ececb253a3a7c39079968f7ad)), closes [#171](https://github.com/kryptamine/postal/issues/171)
* **list:** pick several Threads with J and K and act on them at once ([60718a9](https://github.com/kryptamine/postal/commit/60718a9330f67bec200a1bb6bea7f5647e80317c)), closes [#160](https://github.com/kryptamine/postal/issues/160)
* **mcp:** serve the cache read-only to agents as mcp tools ([d620896](https://github.com/kryptamine/postal/commit/d62089619e5a87d388022b621f71f002674edfbd)), closes [#144](https://github.com/kryptamine/postal/issues/144)
* **release:** attest the build provenance of every archive ([2ebe590](https://github.com/kryptamine/postal/commit/2ebe5902670a5bb7241cdbb59cf26d5a409f4311)), closes [#139](https://github.com/kryptamine/postal/issues/139)
* **release:** publish a release only once its binaries are attached ([a298545](https://github.com/kryptamine/postal/commit/a298545602f9dfb26054c62297cd47bf9ad64840)), closes [#139](https://github.com/kryptamine/postal/issues/139)
* **search:** download every Folder whole and count what is left ([19917f4](https://github.com/kryptamine/postal/commit/19917f432232fb59d7ee82ff90c0f74fdbf1ded3)), closes [#157](https://github.com/kryptamine/postal/issues/157)
* **search:** index the body of every cached Message ([970a8ab](https://github.com/kryptamine/postal/commit/970a8ab484b2318b681ca4e2dc2f03a425089908)), closes [#157](https://github.com/kryptamine/postal/issues/157)
* **search:** show the search index as a share of every Message ([9c4eaf9](https://github.com/kryptamine/postal/commit/9c4eaf901cb0df6bc73dba2220b963cbfc170e67)), closes [#157](https://github.com/kryptamine/postal/issues/157)
* **search:** take operators in the Query and complete them ([c2ae6cf](https://github.com/kryptamine/postal/commit/c2ae6cf12d3c4f5e7cc5cf896a2e7e9a4d38be1f)), closes [#156](https://github.com/kryptamine/postal/issues/156)
* **ui:** cap the height of the help and folder popups and scroll them ([a9352d1](https://github.com/kryptamine/postal/commit/a9352d1c71d49ab7c9ac8d224a65ed61986bd384))
* **ui:** mark the selected Folder in the picker in yellow ([f918182](https://github.com/kryptamine/postal/commit/f9181820e6254653cede2ce0af8874ff81b1c3de)), closes [#165](https://github.com/kryptamine/postal/issues/165)
* **ui:** mark the Settings tab and row in the terminal's yellow ([9aac2f9](https://github.com/kryptamine/postal/commit/9aac2f98a643073f1b5fbe34ae02b5823a9ae459)), closes [#165](https://github.com/kryptamine/postal/issues/165)
* **ui:** scroll the help popup and filter it with / ([9e13e4b](https://github.com/kryptamine/postal/commit/9e13e4b3fb92133ca30d2d07822fd6bbd31bc7b1)), closes [#161](https://github.com/kryptamine/postal/issues/161)
* **ui:** show each Folder's total beside its Unread count ([0e2807f](https://github.com/kryptamine/postal/commit/0e2807f206f45a46619394a02edbb97d42b92fc0)), closes [#164](https://github.com/kryptamine/postal/issues/164)
* **ui:** split Settings into tabs with a hint per setting ([a129976](https://github.com/kryptamine/postal/commit/a1299760d47e3110a497f49aa541f8949fd9f51a)), closes [#165](https://github.com/kryptamine/postal/issues/165)


### Bug Fixes

* **cache:** keep the Picker's total apart from the resync's count ([1b8fb5a](https://github.com/kryptamine/postal/commit/1b8fb5a9552e27dcd2a83b1b6a6c8234baaabd46)), closes [#164](https://github.com/kryptamine/postal/issues/164)
* **cli:** show and take folder names as people read them ([bbdaf6b](https://github.com/kryptamine/postal/commit/bbdaf6b34e44bd499f0134ae8b06afe11d47363f))
* **install:** warn when another postal comes first on path ([6b6d1c5](https://github.com/kryptamine/postal/commit/6b6d1c54ffc14f06d805e3281c6edb1348c8adac))
* **landing:** load the hero still at once and size Copy for a finger ([7018cfc](https://github.com/kryptamine/postal/commit/7018cfcc17ad0107b0ceddb9ef69a98056ca3d0d))
* **release:** build and verify the archives only for a release ([185d171](https://github.com/kryptamine/postal/commit/185d171170f70796b702332d20aade2076590b8b))
* **release:** build macos on intel runners, finish a release by hand ([8f953df](https://github.com/kryptamine/postal/commit/8f953dfcd92df53b152c0df12a215d83bda81554))
* **release:** time out a build after 30 minutes ([a70f608](https://github.com/kryptamine/postal/commit/a70f6081d9146f6255be224517dd6a9e717a8fa2))
* **ui:** offer d unlink only on the Settings tab that unlinks ([6faf4ee](https://github.com/kryptamine/postal/commit/6faf4eeac0e2c0295e831b1b5b8a9cf142ce6726))
* **ui:** set the Settings title into the popup's border ([8f4e58b](https://github.com/kryptamine/postal/commit/8f4e58b232803455e0f91d47428274e88e12a3a6))

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
