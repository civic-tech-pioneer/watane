<!-- Opening. Author: Claude. Target 160 words. Running example per 03-review Turn 3; illustrative, marked as such. -->

"Make the dashboard load faster."

That is the whole request. An agent finds the slow aggregate query and caches
its result per team for ten minutes. The tests pass, the page is quick, and
nothing in the request was ignored. *(This is an illustration, not a real
incident.)*

Two decisions came with that cache, and nobody made either of them on purpose.

The first had already been made. The old query ran once per viewer, with that
viewer's permissions: a manager saw the salary column, an engineer on the same
team did not. Sharing one result per team assumes that team membership is a safe
boundary for sharing. The code said otherwise, and the agent did not look.

The second had never been made. The numbers used to be live; now they can be ten
minutes old, even right after someone's access changes. Whether that is
acceptable is a product question, and nobody asked it.

One is a constraint the agent should have found, the other a question it should
have asked. Both get expensive the same way: through everything built on them
before anyone looks.
