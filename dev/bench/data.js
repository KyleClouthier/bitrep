window.BENCHMARK_DATA = {
  "lastUpdate": 1784322451766,
  "repoUrl": "https://github.com/KyleClouthier/bitrep",
  "entries": {
    "Benchmark": [
      {
        "commit": {
          "author": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "committer": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "distinct": true,
          "id": "4207a1256c6e0d7e92af1ac489ddde25dc77b4ec",
          "message": "Harden OpenSSF Scorecard + org copyright headers\n\n- SECURITY.md, least-privilege workflow permissions, SHA-pinned actions, CodeQL (SAST)\n- codecov.yml threshold so the coverage badge reads green honestly (~80%)\n- documented waiver for two unreachable PyO3 advisories (upgrade to 0.24+ tracked)\n- Clouthier Simulation Labs added to all source-file copyright headers (matches Cairn convention; MIT/Apache license terms unchanged)",
          "timestamp": "2026-07-13T08:19:29-04:00",
          "tree_id": "7bef901d330fd7a4cf25722b6759d62dd0366d09",
          "url": "https://github.com/KyleClouthier/bitrep/commit/4207a1256c6e0d7e92af1ac489ddde25dc77b4ec"
        },
        "date": 1783945406905,
        "tool": "cargo",
        "benches": [
          {
            "name": "sum/naive/1000",
            "value": 877,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000",
            "value": 3678,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000",
            "value": 2383,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000",
            "value": 2764,
            "range": "± 13",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000",
            "value": 2429,
            "range": "± 29",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/100000",
            "value": 93712,
            "range": "± 2756",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/100000",
            "value": 374450,
            "range": "± 458",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/100000",
            "value": 275471,
            "range": "± 2869",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/100000",
            "value": 492616,
            "range": "± 1005",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/100000",
            "value": 510890,
            "range": "± 4780",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/1000000",
            "value": 938626,
            "range": "± 1205",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000000",
            "value": 3749147,
            "range": "± 69643",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000000",
            "value": 2692586,
            "range": "± 21862",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000000",
            "value": 4957864,
            "range": "± 28882",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000000",
            "value": 5068590,
            "range": "± 23827",
            "unit": "ns/iter"
          },
          {
            "name": "merge/100-shards-of-10k",
            "value": 2650,
            "range": "± 4",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "committer": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "distinct": true,
          "id": "66ea3a85a4bd375bbd58c816d8bfc2e07eecc70a",
          "message": "docs: state accumulator capacity as an explicit named limit (2^63 additions)",
          "timestamp": "2026-07-13T08:23:02-04:00",
          "tree_id": "ee3d93fbad031efc69684e256fc7a9e1f9b21355",
          "url": "https://github.com/KyleClouthier/bitrep/commit/66ea3a85a4bd375bbd58c816d8bfc2e07eecc70a"
        },
        "date": 1783945597393,
        "tool": "cargo",
        "benches": [
          {
            "name": "sum/naive/1000",
            "value": 771,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000",
            "value": 3224,
            "range": "± 22",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000",
            "value": 1979,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000",
            "value": 2378,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000",
            "value": 2140,
            "range": "± 51",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/100000",
            "value": 81808,
            "range": "± 47",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/100000",
            "value": 327260,
            "range": "± 488",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/100000",
            "value": 171388,
            "range": "± 275",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/100000",
            "value": 416116,
            "range": "± 1073",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/100000",
            "value": 437458,
            "range": "± 575",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/1000000",
            "value": 819665,
            "range": "± 615",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000000",
            "value": 3276463,
            "range": "± 1448",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000000",
            "value": 1705677,
            "range": "± 5935",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000000",
            "value": 4255834,
            "range": "± 7342",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000000",
            "value": 4400248,
            "range": "± 5753",
            "unit": "ns/iter"
          },
          {
            "name": "merge/100-shards-of-10k",
            "value": 2312,
            "range": "± 3",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "committer": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "distinct": true,
          "id": "b489d3800d9226ee2f3a5998d57ea5c37e92fa40",
          "message": "Simplify FloatGCounter proofs: drop redundant ejoin wrappers\n\nPer Lean Zulip feedback (Yan Yablonovskiy): the ejoin_comm/assoc/idem\nwrapper theorems were redundant restatements of core Nat.max lemmas.\njoin_comm/assoc/idem now defer directly to Nat.max_comm/assoc/self. Fewer\nhoops for a reviewer, same proof, still Lean-core-only with zero sorry.\nComparator theorem list unchanged (only ever referenced the join_* CRDT laws).",
          "timestamp": "2026-07-13T08:49:43-04:00",
          "tree_id": "3474d9cb03b98a9b2e0234120655aa44a39d7a7d",
          "url": "https://github.com/KyleClouthier/bitrep/commit/b489d3800d9226ee2f3a5998d57ea5c37e92fa40"
        },
        "date": 1783947205162,
        "tool": "cargo",
        "benches": [
          {
            "name": "sum/naive/1000",
            "value": 877,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000",
            "value": 3679,
            "range": "± 51",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000",
            "value": 2425,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000",
            "value": 2878,
            "range": "± 46",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000",
            "value": 2404,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/100000",
            "value": 93711,
            "range": "± 97",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/100000",
            "value": 374348,
            "range": "± 317",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/100000",
            "value": 271628,
            "range": "± 2040",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/100000",
            "value": 490938,
            "range": "± 4053",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/100000",
            "value": 510750,
            "range": "± 1055",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/1000000",
            "value": 938350,
            "range": "± 933",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000000",
            "value": 3744762,
            "range": "± 53372",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000000",
            "value": 2737134,
            "range": "± 47347",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000000",
            "value": 4956668,
            "range": "± 25071",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000000",
            "value": 5063404,
            "range": "± 8912",
            "unit": "ns/iter"
          },
          {
            "name": "merge/100-shards-of-10k",
            "value": 2650,
            "range": "± 4",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "committer": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "distinct": true,
          "id": "f3a968a4fdc2e924d15409a37361cc1dd68b1aba",
          "message": "docs: cite the Lean manual's Validating a Lean Proof page for the comparator layer",
          "timestamp": "2026-07-13T08:58:28-04:00",
          "tree_id": "ada13bb64e8ffc25833bee329eeeb65b7ea29936",
          "url": "https://github.com/KyleClouthier/bitrep/commit/f3a968a4fdc2e924d15409a37361cc1dd68b1aba"
        },
        "date": 1783947728754,
        "tool": "cargo",
        "benches": [
          {
            "name": "sum/naive/1000",
            "value": 877,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000",
            "value": 3677,
            "range": "± 22",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000",
            "value": 2381,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000",
            "value": 2820,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000",
            "value": 2433,
            "range": "± 20",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/100000",
            "value": 93680,
            "range": "± 1315",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/100000",
            "value": 374214,
            "range": "± 659",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/100000",
            "value": 269347,
            "range": "± 771",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/100000",
            "value": 491477,
            "range": "± 11836",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/100000",
            "value": 509662,
            "range": "± 1207",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/1000000",
            "value": 937096,
            "range": "± 1115",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000000",
            "value": 3744116,
            "range": "± 3915",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000000",
            "value": 2679834,
            "range": "± 14938",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000000",
            "value": 4955000,
            "range": "± 7195",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000000",
            "value": 5052206,
            "range": "± 124288",
            "unit": "ns/iter"
          },
          {
            "name": "merge/100-shards-of-10k",
            "value": 2648,
            "range": "± 8",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "committer": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "distinct": true,
          "id": "203d3474e74048a6a3d4a15e8a2583c4cd327dab",
          "message": "release: v0.4.0 (RelSketch quantile sketch)\n\nBump crate/bindings to 0.4.0 and cut the CHANGELOG entry: the reproducible\nbyte-identical relative-error quantile sketch (RelSketch), its Lean-proved\nmerge laws, OTel/Prometheus histogram correspondence, and the two decoder\nhardening fixes from the fuzz soak (equality bitwise-on-every-field; reject\nout-of-range bucket keys in from_bytes and from_otel).",
          "timestamp": "2026-07-14T13:35:56-04:00",
          "tree_id": "081090810b7c758a9568996115bbc5d2e8645c16",
          "url": "https://github.com/KyleClouthier/bitrep/commit/203d3474e74048a6a3d4a15e8a2583c4cd327dab"
        },
        "date": 1784050997783,
        "tool": "cargo",
        "benches": [
          {
            "name": "sum/naive/1000",
            "value": 770,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000",
            "value": 3223,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000",
            "value": 1978,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000",
            "value": 2464,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000",
            "value": 2135,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/100000",
            "value": 81795,
            "range": "± 253",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/100000",
            "value": 327317,
            "range": "± 1104",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/100000",
            "value": 171584,
            "range": "± 179",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/100000",
            "value": 404438,
            "range": "± 6430",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/100000",
            "value": 437307,
            "range": "± 1509",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/1000000",
            "value": 819411,
            "range": "± 372",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000000",
            "value": 3276729,
            "range": "± 2384",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000000",
            "value": 1704431,
            "range": "± 3319",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000000",
            "value": 4257151,
            "range": "± 8647",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000000",
            "value": 4398814,
            "range": "± 77583",
            "unit": "ns/iter"
          },
          {
            "name": "merge/100-shards-of-10k",
            "value": 2307,
            "range": "± 5",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "committer": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "distinct": true,
          "id": "a7e0f76a9704be6bcfac12f7456834a823b5aa6e",
          "message": "style: rustfmt the key-space regression test\n\ncargo fmt --check failed on the assert! in out_of_range_bucket_key_...;\nno functional change (test whitespace only). crates.io 0.4.0 unaffected.",
          "timestamp": "2026-07-14T14:02:14-04:00",
          "tree_id": "9211b74de4401a7ea4d46ea3a147a8888e2031bd",
          "url": "https://github.com/KyleClouthier/bitrep/commit/a7e0f76a9704be6bcfac12f7456834a823b5aa6e"
        },
        "date": 1784052363464,
        "tool": "cargo",
        "benches": [
          {
            "name": "sum/naive/1000",
            "value": 877,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000",
            "value": 3680,
            "range": "± 45",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000",
            "value": 2383,
            "range": "± 12",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000",
            "value": 2777,
            "range": "± 45",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000",
            "value": 2403,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/100000",
            "value": 93750,
            "range": "± 126",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/100000",
            "value": 374596,
            "range": "± 433",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/100000",
            "value": 227858,
            "range": "± 1046",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/100000",
            "value": 492198,
            "range": "± 4212",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/100000",
            "value": 510436,
            "range": "± 1818",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/1000000",
            "value": 937873,
            "range": "± 4179",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000000",
            "value": 3745461,
            "range": "± 123555",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000000",
            "value": 2278728,
            "range": "± 12754",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000000",
            "value": 4953842,
            "range": "± 13456",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000000",
            "value": 5058321,
            "range": "± 40348",
            "unit": "ns/iter"
          },
          {
            "name": "merge/100-shards-of-10k",
            "value": 2646,
            "range": "± 31",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "committer": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "distinct": true,
          "id": "9391214276a3db4849d87dca7bf2f60b2a670394",
          "message": "docs: tidy example labels and drop orphaned version tag\n\nRename 'Probe:' -> 'Example:' in three example doc headers (they are\nself-contained demos, not the exploratory sense) and drop the 'v0.3'\nqualifier on the toolkit test (the toolkit shipped in v0.2; there was no\n0.3.0 release). Doc-comment only; no functional change.",
          "timestamp": "2026-07-14T14:46:57-04:00",
          "tree_id": "9596121c858551b8f6987c84fe9a980e08c0a9c3",
          "url": "https://github.com/KyleClouthier/bitrep/commit/9391214276a3db4849d87dca7bf2f60b2a670394"
        },
        "date": 1784055034907,
        "tool": "cargo",
        "benches": [
          {
            "name": "sum/naive/1000",
            "value": 994,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000",
            "value": 4156,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000",
            "value": 2552,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000",
            "value": 3086,
            "range": "± 44",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000",
            "value": 2695,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/100000",
            "value": 105486,
            "range": "± 1958",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/100000",
            "value": 422038,
            "range": "± 932",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/100000",
            "value": 221155,
            "range": "± 312",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/100000",
            "value": 536251,
            "range": "± 5049",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/100000",
            "value": 563941,
            "range": "± 1640",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/1000000",
            "value": 1057289,
            "range": "± 2258",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000000",
            "value": 4225971,
            "range": "± 20694",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000000",
            "value": 2197997,
            "range": "± 4173",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000000",
            "value": 5828016,
            "range": "± 7111",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000000",
            "value": 5674451,
            "range": "± 6383",
            "unit": "ns/iter"
          },
          {
            "name": "merge/100-shards-of-10k",
            "value": 2961,
            "range": "± 8",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "committer": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "distinct": true,
          "id": "8fc563e1573390dcd85720601b7ec8641540062c",
          "message": "release: v0.4.1 — docs & polish\n\nREADME leads with the full surface (sums, dot products, statistics,\nreproducible quantiles) and the 'sign a p99' line; example labels and the\ntoolkit version tag tidied. No functional change.",
          "timestamp": "2026-07-14T15:38:26-04:00",
          "tree_id": "9d8d4b32a32dd461ba1cc28866e125ebd33c851f",
          "url": "https://github.com/KyleClouthier/bitrep/commit/8fc563e1573390dcd85720601b7ec8641540062c"
        },
        "date": 1784058128169,
        "tool": "cargo",
        "benches": [
          {
            "name": "sum/naive/1000",
            "value": 877,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000",
            "value": 3677,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000",
            "value": 2387,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000",
            "value": 2766,
            "range": "± 20",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000",
            "value": 2406,
            "range": "± 15",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/100000",
            "value": 93631,
            "range": "± 9024",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/100000",
            "value": 374159,
            "range": "± 402",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/100000",
            "value": 499774,
            "range": "± 9746",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/100000",
            "value": 491319,
            "range": "± 2540",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/100000",
            "value": 509216,
            "range": "± 2525",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/1000000",
            "value": 936544,
            "range": "± 1292",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000000",
            "value": 3742212,
            "range": "± 2127",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000000",
            "value": 2722901,
            "range": "± 7910",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000000",
            "value": 4955958,
            "range": "± 9565",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000000",
            "value": 5044875,
            "range": "± 7371",
            "unit": "ns/iter"
          },
          {
            "name": "merge/100-shards-of-10k",
            "value": 2661,
            "range": "± 6",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "committer": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "distinct": true,
          "id": "96ee19a4b44423602860b7dba45c4701232022e7",
          "message": "release: v0.4.2 — expose count() in Python and JS bindings\n\nThe number of accumulated values is already part of the serialized state\nand available on the Rust core, but the Python/wasm bindings did not expose\nit, forcing callers to infer the count indirectly (unreliable for\nconstant-valued data). Add count() to SumF64, SumF32, FastSumF64,\nMomentsF64, Moments4F64, CovF64, and WeightedMomentsF64 in both bindings.\nNo change to the Rust core, the byte format, or any golden/conformance\nvector.",
          "timestamp": "2026-07-16T09:56:02-04:00",
          "tree_id": "620758ac3fd1b5fbeeae82a40ff4812091a3c782",
          "url": "https://github.com/KyleClouthier/bitrep/commit/96ee19a4b44423602860b7dba45c4701232022e7"
        },
        "date": 1784210392176,
        "tool": "cargo",
        "benches": [
          {
            "name": "sum/naive/1000",
            "value": 877,
            "range": "± 49",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000",
            "value": 3678,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000",
            "value": 2254,
            "range": "± 15",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000",
            "value": 2708,
            "range": "± 14",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000",
            "value": 2488,
            "range": "± 384",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/100000",
            "value": 93574,
            "range": "± 185",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/100000",
            "value": 374351,
            "range": "± 2912",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/100000",
            "value": 221281,
            "range": "± 6584",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/100000",
            "value": 505540,
            "range": "± 1612",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/100000",
            "value": 495886,
            "range": "± 947",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/1000000",
            "value": 937073,
            "range": "± 1014",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000000",
            "value": 3744366,
            "range": "± 3817",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000000",
            "value": 2217127,
            "range": "± 9453",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000000",
            "value": 5098444,
            "range": "± 12243",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000000",
            "value": 4913778,
            "range": "± 164494",
            "unit": "ns/iter"
          },
          {
            "name": "merge/100-shards-of-10k",
            "value": 2642,
            "range": "± 7",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "committer": {
            "email": "kyleclouthier83@gmail.com",
            "name": "Kyle Clouthier",
            "username": "KyleClouthier"
          },
          "distinct": true,
          "id": "03274bc520a82057f252e7d9391091c65a766b04",
          "message": "v0.5.0 — the exact tier: group subtraction and correctly rounded regression\n\nSumF64::try_unmerge: exact removal of a merged contribution (two's-complement\nborrow chain; refuses sticky NaN/inf flags and count underflow, state\nuntouched on refusal). CovMatrixF64::try_sub: all-or-nothing exact downdating\nof the full second-moment state — leave-one-out, influence analysis, and\nverifiable unlearning for least-squares models, at any removal fraction and\nconditioning. CovMatrixF64::try_regression_exact: normal equations from the\nstate's exact integers, solved by Cramer with fraction-free Bareiss\ndeterminants, one correct rounding per coefficient — bits defined by the\nmathematics, identical on any machine; exact singularity reported as\nDegenerate instead of a blurred answer.\n\nVerification matched claim-for-claim: Kani proves the merge/unmerge group\ninverse and refusal safety at the bit level for all valid states; Lean proves\nthe model-level inverse (unmerge_inverts_merge, lsum_unmerge; zero sorry,\nstandard axiom base); regression_exact is verified against an independent\nexact rational oracle (different algorithm, bit-compared) and a new\ncoverage-guided fuzz target (sub_roundtrip, 3.2M execs clean) whose corpus\nincludes the input demonstrating the underflow-flag refusal path.\n\nPython bindings gain try_unmerge / sub / regression_exact. Byte format\nunchanged; all existing vectors unaffected.",
          "timestamp": "2026-07-17T17:03:10-04:00",
          "tree_id": "efa1ac0f6984d133195081eeb47b5669c286ff3e",
          "url": "https://github.com/KyleClouthier/bitrep/commit/03274bc520a82057f252e7d9391091c65a766b04"
        },
        "date": 1784322451343,
        "tool": "cargo",
        "benches": [
          {
            "name": "sum/naive/1000",
            "value": 877,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000",
            "value": 3679,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000",
            "value": 2256,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000",
            "value": 2720,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000",
            "value": 2486,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/100000",
            "value": 93654,
            "range": "± 185",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/100000",
            "value": 374177,
            "range": "± 463",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/100000",
            "value": 222064,
            "range": "± 918",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/100000",
            "value": 505359,
            "range": "± 1544",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/100000",
            "value": 495034,
            "range": "± 1298",
            "unit": "ns/iter"
          },
          {
            "name": "sum/naive/1000000",
            "value": 937101,
            "range": "± 5572",
            "unit": "ns/iter"
          },
          {
            "name": "sum/kahan/1000000",
            "value": 3742550,
            "range": "± 1880",
            "unit": "ns/iter"
          },
          {
            "name": "sum/xsum/1000000",
            "value": 2178465,
            "range": "± 11647",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep/1000000",
            "value": 5097768,
            "range": "± 13812",
            "unit": "ns/iter"
          },
          {
            "name": "sum/bitrep_fast/1000000",
            "value": 4904330,
            "range": "± 14516",
            "unit": "ns/iter"
          },
          {
            "name": "merge/100-shards-of-10k",
            "value": 2638,
            "range": "± 45",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}