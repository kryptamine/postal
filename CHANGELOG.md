# Changelog

## [0.7.0](https://github.com/kryptamine/postal/compare/v0.6.0...v0.7.0) (2026-11-25)


### Features

* **nvim:** send a Draft from the editor with :PostalSend ([17180e9](https://github.com/kryptamine/postal/commit/17180e936be366352eac9e2893d377ba881633af)), closes [#218](https://github.com/kryptamine/postal/issues/218)
* **ui:** unfold a Message that joins the open Thread, marked new ([2ef4111](https://github.com/kryptamine/postal/commit/2ef4111e6264c5e63ad1cd696cac908b35ee5eb5)), closes [#222](https://github.com/kryptamine/postal/issues/222)


### Bug Fixes

* **cli:** show the text of a plain part that carries HTML ([a66bb28](https://github.com/kryptamine/postal/commit/a66bb2868a94071a05b43488e5da42c549896ae2))
* **imap:** cut a Snippet without the HTML a plain part carries ([b32a062](https://github.com/kryptamine/postal/commit/b32a062dee46cb9aa67c492a2bd7b840d84d8fba))
* **imap:** show a Message IDLE brings in once its Snippet is cut ([9a4d59d](https://github.com/kryptamine/postal/commit/9a4d59d66e70217ab5fee7e76b4cba0dc7dab639)), closes [#222](https://github.com/kryptamine/postal/issues/222)
* **ui:** close a Draft's preview with q or Esc, discard it with d ([b0479d2](https://github.com/kryptamine/postal/commit/b0479d2f3f4b89e8778d839bd446ea8557ef6f1e))

## [0.6.0](https://github.com/kryptamine/postal/compare/v0.5.0...v0.6.0) (2026-11-25)


### Features

* **cli:** register postal as the mailto: handler ([0341a0e](https://github.com/kryptamine/postal/commit/0341a0e016839a5d730b45a2e1a41848d94a71cf)), closes [#190](https://github.com/kryptamine/postal/issues/190)
* **cli:** show the Thread of a Message file with thread show --message ([3ae5f69](https://github.com/kryptamine/postal/commit/3ae5f69b6bbd1ca4f5bddb67c8e9c3b324f755ce)), closes [#181](https://github.com/kryptamine/postal/issues/181)
* **cli:** sync a Folder from the CLI and MCP with no TUI open ([d49ad32](https://github.com/kryptamine/postal/commit/d49ad32e4bd4f5e17158f375aaa5340af6d6669f)), closes [#184](https://github.com/kryptamine/postal/issues/184) [#69](https://github.com/kryptamine/postal/issues/69)
* **compose:** open a Draft from a mailto: link ([3b022c0](https://github.com/kryptamine/postal/commit/3b022c0f227131718f4740b0ec0eaf39d2496fac)), closes [#189](https://github.com/kryptamine/postal/issues/189)
* **compose:** send a Markdown Draft's HTML beside its text ([0c33304](https://github.com/kryptamine/postal/commit/0c3330480fdb35647462b0b0534a1a221bd7499c)), closes [#197](https://github.com/kryptamine/postal/issues/197)
* **contacts:** read macOS Contacts as a source of Correspondents ([8c11913](https://github.com/kryptamine/postal/commit/8c11913e8639682f0cfe7c2bc0b7594d62d874e1)), closes [#191](https://github.com/kryptamine/postal/issues/191)
* **nvim:** add a Correspondent to a Draft with :PostalContacts ([19f5fc6](https://github.com/kryptamine/postal/commit/19f5fc6174f04795f498a022766489e5a66860fd)), closes [#181](https://github.com/kryptamine/postal/issues/181)
* **nvim:** bring back what was sent to an agent with &lt;C-p&gt; and &lt;C-n&gt; ([307e77c](https://github.com/kryptamine/postal/commit/307e77c2b629bbc1b5d22182890dd71f39aa4be6)), closes [#181](https://github.com/kryptamine/postal/issues/181)
* **nvim:** diagnose a Draft's To: and Cc: as they are written ([5ecca3a](https://github.com/kryptamine/postal/commit/5ecca3a98d2135fa388797321c9047422733d4cf)), closes [#181](https://github.com/kryptamine/postal/issues/181)
* **nvim:** fold the Quote a Draft or a Message ends in ([00d743a](https://github.com/kryptamine/postal/commit/00d743a7c6cd91124998c4bdc6b6ddcc1fb8c900)), closes [#181](https://github.com/kryptamine/postal/issues/181)
* **nvim:** hand the agent a Message's Thread with the Message ([8e683ad](https://github.com/kryptamine/postal/commit/8e683add52b864c2dcd980735cd7434e82f4914b)), closes [#181](https://github.com/kryptamine/postal/issues/181)
* **nvim:** keep a typed paragraph on one line, and spell check on ask ([539fe21](https://github.com/kryptamine/postal/commit/539fe21a87761848316de02845fdf783632f7be8)), closes [#181](https://github.com/kryptamine/postal/issues/181)
* **nvim:** let setup() map each key of the pane and the review ([20962ba](https://github.com/kryptamine/postal/commit/20962ba4e715145c0551cbfb52b642fe32153e4c))
* **nvim:** open a Message's link from its [n] marker with gx ([ded8310](https://github.com/kryptamine/postal/commit/ded8310093f8c593906c7abc2003d9553e7e3789)), closes [#181](https://github.com/kryptamine/postal/issues/181)
* **nvim:** parse a Markdown Draft's body as Markdown ([4af579c](https://github.com/kryptamine/postal/commit/4af579caeb5ef30f06b8794895422749c3bdf816)), closes [#197](https://github.com/kryptamine/postal/issues/197)
* **nvim:** send a prompt set up by name with :PostalAgent &lt;name&gt; ([ab55ad1](https://github.com/kryptamine/postal/commit/ab55ad1c9835ef6bcfc5d37ddec28d886c2c62de)), closes [#181](https://github.com/kryptamine/postal/issues/181)
* **nvim:** show each Attach: file's size and open it with gf ([d5402ac](https://github.com/kryptamine/postal/commit/d5402ac5c246ac48e86abe7a2195add272114de0))
* **nvim:** tell in checkhealth a postal of another release than the plugin ([aaeeb77](https://github.com/kryptamine/postal/commit/aaeeb77363c0607bc77e58bd7f9df57ed7e97a48)), closes [#181](https://github.com/kryptamine/postal/issues/181)
* **nvim:** warn of an attachment the body mentions and nothing attaches ([451338d](https://github.com/kryptamine/postal/commit/451338d9ddccfde22f7c086fc42f73c3c1584bfc)), closes [#181](https://github.com/kryptamine/postal/issues/181)
* **preview:** show a meeting invitation as a card ([d77402d](https://github.com/kryptamine/postal/commit/d77402d822d18a3c96453a7de622e89bdffd50e0)), closes [#196](https://github.com/kryptamine/postal/issues/196)
* **send:** hold a Send for a few seconds so u can take it back ([9229243](https://github.com/kryptamine/postal/commit/9229243939950527cbd198f8488e952d1b99c785)), closes [#195](https://github.com/kryptamine/postal/issues/195)
* **ui:** fetch new mail now with ^r ([39062c4](https://github.com/kryptamine/postal/commit/39062c4cee078c479ba57e12c5b0ca0ac97b1e41))
* **ui:** take the session over from a TUI open in another terminal ([0335c2a](https://github.com/kryptamine/postal/commit/0335c2a22cd4921f946ba867d066ff958217bff4)), closes [#201](https://github.com/kryptamine/postal/issues/201)


### Bug Fixes

* **cli:** keep the sync lock beside the Outbox it guards ([0d3cd9c](https://github.com/kryptamine/postal/commit/0d3cd9c769be94b41fc8e1ddecab8ed5e14c91cf)), closes [#201](https://github.com/kryptamine/postal/issues/201)
* **imap:** number a refused page of older Messages again ([2afb729](https://github.com/kryptamine/postal/commit/2afb72990ee4a264ad5352611753df1069131a7e))
* **imap:** read UID FETCH, STORE and SEARCH by hand ([a6e17b7](https://github.com/kryptamine/postal/commit/a6e17b79a2698710fa3373d79fe986d8d3f1b3b1)), closes [#213](https://github.com/kryptamine/postal/issues/213)
* **imap:** reconnect after any failure but a refused Account ([27865e6](https://github.com/kryptamine/postal/commit/27865e6889c95434a345ca60dfdfca82290c0c13))
* **imap:** resync a cached Folder once a move lands in it ([30f0e44](https://github.com/kryptamine/postal/commit/30f0e448a4b1a70d1e70a9036241d213490c2c3d))
* **imap:** take in the flag changes a STORE's answer carries ([6c74c42](https://github.com/kryptamine/postal/commit/6c74c42f113518b0d66083052732d54fbacf8c3d))
* **rendered:** centre a narrow block under an inherited align=center ([fe2e9f5](https://github.com/kryptamine/postal/commit/fe2e9f58a8d5c00ca6f9f405e68ce88b32ebaa8d))
* **ui:** keep the render thread alive through a panic on one mail ([358a580](https://github.com/kryptamine/postal/commit/358a5803817373fabd74a2fea961cb941935bcb5))
* **ui:** toast a cache error in answer to an event instead of exiting ([7fb9807](https://github.com/kryptamine/postal/commit/7fb980716e5c986d754b365adc6ecb8d15adc6ab))

## [0.5.0](https://github.com/kryptamine/postal/compare/v0.4.0...v0.5.0) (2026-11-24)


### Features

* **landing:** add a not-found page kept out of the index ([5c45c65](https://github.com/kryptamine/postal/commit/5c45c650d53f9c2330ba03f9a6c976629963ffbd))
* **landing:** add robots.txt and a sitemap ([046357b](https://github.com/kryptamine/postal/commit/046357bb18fd80c950dd533641f3c7902c0a0e85))
* **landing:** name what postal is in the title, h1 and json-ld ([57ee7d6](https://github.com/kryptamine/postal/commit/57ee7d6ca68c029f85abdcd3865028a381e1a147))


### Performance Improvements

* **cache:** check the whole cache only after a session that broke off ([54ef895](https://github.com/kryptamine/postal/commit/54ef8956780f60b12d388ba52506b8518b4a970d))
* **oauth:** buy the access token on the first connection, not at start ([a344723](https://github.com/kryptamine/postal/commit/a3447238b38fa8acb39616f8cfc5d904ab0defe7))

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
