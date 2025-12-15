# RUJIRA Template

This readme is an example of how to setup your repo correctly to work with THORChain. In your case, it should describe how your contract works, links to audits, docs and more. 

# Implementation

Eg: This is how it works

## Whitelisting on Base Layer

Open a PR into:
```common/wasmpermissions/wasm_permissions_mainnet.go```

Checksum: contract checksum
Origin: link to this repo
Deployers: array of whitelisted deployer addresses

eg: https://gitlab.com/thorchain/thornode/-/merge_requests/4101/diffs#bde29e6b0f5973e3ffbd8f10cb64bd253f2d39ad
```
		// nami-index-nav v1.0.0
		"d20dc480a8484242f72c7f1e8db0bc39e5da48f93a4cc4fa679d9e8acff65a62": {
			Origin: "https://github.com/NAMIProtocol/nami-contracts/tree/3efb8706f2438323d5dbae29c337a11a6509de30/contracts/nami-index-nav",
			Deployers: map[string]bool{
				"thor1zjwanvezcjp6hefgt6vqfnrrdm8yj9za3s8ss0": true,
			},
		},
```

This will then be returned on the thorchain endpoint:

https://thornode.ninerealms.com/thorchain/codes
```json
{"code":"11ddc91557ec8ea845b74ceb6b9f5502672e8a856b0c1752eb0ce19e3ad81dac",
"deployers":["thor1e0lmk5juawc46jwjwd0xfz587njej7ay5fh6cd"],
"origin":"https://gitlab.com/thorchain/rujira/-/tree/8cc96cf59037a005051aff2fd16e46ff509a9241/contracts/rujira-fin"}
```

### Cargo.toml

THORChain explorers will query the origin URL and retrieve values on the cargo.toml file. In particular:

```yaml
[package]
name = "rujira-template" //contract name
version = "1.0.0" //keep this up to date
authors = [] //handles
audits = ["link/to/your/audit"]
auditors = ["NAME"]
docs = "link/to/your/docs"
```

This will then be displayed here
https://thorchain.net/rujira/contracts

