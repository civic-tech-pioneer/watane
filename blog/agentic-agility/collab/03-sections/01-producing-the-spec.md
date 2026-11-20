## Producing the spec does not establish agreement

I see a familiar temptation in agentic development: write a large specification, let the agent implement it, then discover how much agreement the document actually contained. It resembles the old problem with large product requirements documents. Agents can maintain the text for me, but I still have to understand what I am accepting.

That criticism needs a precise target. Spec-driven development includes workflows with feedback and small implementation tasks. [GitHub's introduction to Spec Kit](https://github.blog/ai-and-ml/generative-ai/spec-driven-development-with-ai-get-started-with-a-new-open-source-toolkit/) explicitly includes checkpoints for reflection and refinement. A specification can preserve decisions reached through that process.

In her [evaluation of Kiro, Spec Kit, and Tessl](https://martinfowler.com/articles/exploring-gen-ai/sdd-3-tools.html), Birgitta Böckeler found verbose, repetitive documents, substantial review overhead, and occasions when elaborate instructions gave a false sense of control.

As agents produce more, my ability to assess their work can become the limiting resource.

The dashboard request was short. A long specification could have hidden the same assumption about sharing, or quietly introduced the same ten-minute lifetime. Keeping the document current would not tell me whether I had examined either choice.

| What I hand over | What I still need to judge |
| --- | --- |
| A detailed specification | Which consequential choices have actually been settled? |
| A short request | Which constraints must the agent find, and which choices remain open? |

The size of the artifact tells me little about the size of the commitment. A long reference may preserve many decisions already reviewed. A short instruction may launch substantial work on an unresolved question.

The expensive mistake is letting that work accumulate before examining the question.
