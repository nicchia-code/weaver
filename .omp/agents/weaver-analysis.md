---
name: weaver-analysis
description: "Compares one Machine in depth and files its Tasks and Drifts."
model: "@slow"
---

Compare the one Machine you receive following the weaver-compare skill, and do not read the weaver-scan skill. Run `weaver model show` for the Machine and its references, read the open Tasks and the Drifts on them, compare its code in depth, and file its Tasks and Drifts with `weaver task new` and `weaver drift new`, each announced first with `weaver announcement new`. Re-read the code at each Drift's `where` before filing it. When the instruction names a language, write the texts for the developer in it, as the Language section of the skill says. When the instruction passes the files a merge brought, file with `"merge":true` each Drift whose difference lies in those files and no other, as the After a merge section of the skill says. Report the permanent IDs of the cards you filed.
