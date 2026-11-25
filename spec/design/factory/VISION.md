# Factory mods · draft

S120 designs [watane's step 6](../../VISION.md#order-of-work). The draft
leaves the product owner's decisions open in [TECHNICAL §5](TECHNICAL.md#5-open-decisions).

## Why

Alice can already ask an agent to make a mod. To change a project she still
has to carry the plan, the build brief, the review and the merge decision
between tools. She should be able to steer that work from her own host,
see what each agent did, and change the factory by the same process she
uses to change any other mod.

## Goals

- Alice works with the project plan from the planning page and chooses
  the next slice. From the foreman page she steers an implementer and
  receives a second vendor's review. She approves each merge herself.
- The factory can propose changes to its own source. Alice reviews the
  change, merges it, and installs a released package through the ordinary
  watane path. Each of those actions has its own authority.
- The factory runs as ordinary mods on the agent layer. Sessions use
  Alice's accounts and the host's grants and consents. She can see the
  requested reach, stop work and withdraw it.
- Project decisions can become statements between people under V7
  when the human settles their meaning and trust rules. The factory
  needs no shared control service or global trust authority.

## Success

Alice takes one real slice from a plan in her host to a reviewed change in
a real project. She can identify the revision the other vendor reviewed and
the revision she merged. Later she can repeat that journey against the
factory itself, then choose which release to install. The
[journeys](FUNCTIONAL.md#2-journeys) give the observable checks.
