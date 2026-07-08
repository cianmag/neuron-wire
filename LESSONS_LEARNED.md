# Lessons Learned

> Engineering reflections from building neuron-wire at [Zylvon](https://zylvon.com).
> Not everything that counts can be measured, and not everything that can be measured counts.

---

## My original hypothesis failed. That was the best thing that happened to this project.

I started neuron-wire because I thought Sparse Gradient Aging would reduce DHT maintenance bandwidth compared to fixed-interval pinging. The idea made intuitive sense: if you ping stale peers more often and fresh peers less often, you send fewer total pings while maintaining the same routing quality.

The experiments showed the opposite. SGA increased bandwidth by 1.9–2.45× across every tested configuration.

If I had stopped there, that would have been a failure. But I ran the experiment because I wanted to know the answer, not because I wanted to confirm my opinion. The surprising finding — that maintenance pings don't measurably improve routing quality once k-buckets are saturated — only emerged because I was willing to be wrong about my own idea.

**Lesson:** Formulate a hypothesis you can disprove. If it gets disproved, you've still learned something. If you only design experiments you know will work, you're not doing research — you're doing demo engineering.

---

## Reproducibility is not optional. It's the whole point.

Early in the project, I ran benchmarks, got numbers, and moved on. When I came back weeks later, I couldn't reproduce my own results because I hadn't recorded the seed, the config, or the command.

That was a wake-up call. If *I* couldn't reproduce my own work, how could anyone else? And if no one could verify the claims, the claims were worthless.

So I rebuilt the benchmarking infrastructure around determinism: fixed seeds, paper mode, CSV output checked into the repo, CI validation against known-good values. Now anyone can run the same command and get the same numbers. The surprising SGA finding isn't a story I tell — it's a computation anyone can repeat.

**Lesson:** The reproducibility infrastructure is more valuable than any single experimental result. It turns claims into evidence, and it catches regressions when you change something you thought was unrelated.

---

## Negative results are publishable. Engineering projects that only report successes are hiding something.

Every project has things that didn't work. The question is whether you document them or bury them.

I chose to document. The `results/` directory contains benchmark data showing exactly which experiments worked and which didn't. The `FOUNDATIONAL_QNA.md` says "not measured", "untested", and "hypothesis" in dozens of places. The security section says "future work" for every critical protection.

This felt vulnerable at first. Wouldn't it look better to only show the successes?

No. Reviewers and professors have seen hundreds of projects that claim everything works perfectly. They trust the ones that admit what they don't know. A document that says "here are our limitations" is more persuasive than one that pretends limitations don't exist.

**Lesson:** Trust is built by what you admit, not what you claim. If you hide your failures, you're hiding the evidence that you know how to do research.

---

## Engineering and science are different skills. I had to learn the second one from scratch.

I knew how to build things. Rust, async runtimes, UDP transport, DHT routing — those came naturally. I could ship working code.

But building something that works is not the same as generating evidence that it works. The engineering mindset says "make it compile, make it fast, ship it." The research mindset says "formulate a hypothesis, control your variables, measure the outcome, publish the data."

I had to explicitly switch between these modes. The first version of this project was all engineering — look at what I built! The current version is more balanced — here's what I tested, here's what I found, here's what I still don't know.

**Lesson:** If you only have engineering skills, you can build impressive demos. If you only have research skills, you can write papers about things that don't exist. You need both to build something that works *and* generate evidence that it matters.

---

## Plain English is harder than jargon.

When I first wrote the Q&A document, I used terms like "Kademlia-over-UDP substrate with embedded Hebbian learning" because that's how I thought I was supposed to sound. Technical, precise, impressive.

But if someone has to look up three terms in the first sentence, they've already stopped reading.

Rewriting the document with parenthetical explanations and a glossary table was harder than writing the original. It forced me to actually understand what I was talking about well enough to explain it to someone with no background. "DHT (a distributed phonebook no single person controls)" is more work to write than "DHT," but it communicates more.

**Lesson:** If you can't explain it to your grandmother, you don't understand it well enough. And if your document is only readable by people who already know the field, you're not communicating — you're signaling.

---

## The north star question changed everything.

Before this review cycle, I would have told you the project was about "decentralized neural computation" or "P2P learning runtimes." Those are descriptions of what the system does, not what capability it unlocks.

When the reviewer asked for a concrete capability — "what becomes possible that is not practical today?" — I didn't have a good answer. I had architectural descriptions, not use cases.

The answer I settled on — "zero-infrastructure collaborative learning" — reframes every design decision. Would someone in a village with a phone be able to join this network? If not, why not? What's the bottleneck? What's the minimum viable deployment?

This question is harder to answer than "what does your architecture look like?" But it's also the question that makes people care.

**Lesson:** Architecture is table stakes. The capability is the point. If someone finishes your paper knowing only that you built an interesting architecture, you've failed. They should finish knowing what new thing is now possible.

---

## The hardest bug was a borrow checker problem. The hardest problem is still unsolved.

The worst bug in the codebase was a deadlock in `periodic_maintenance` that held a mutable borrow while calling `ping_node`. The fix was `std::mem::take` — pre-collect the ping targets before entering the borrow scope.

That bug took hours to find and 30 seconds to fix. It was hard, but it was *knowable* — there was a right answer, and the borrow checker would tell me when I found it.

The hardest problem in the project is not like that. It's "does this actually work over real Internet conditions?" That question has no borrow checker. There's no compiler error when your answer is wrong. The only way to find out is to deploy across continents and measure what happens.

**Lesson:** Technical bugs are finite. Research questions are open. The distinction is important because it tells you what kind of work you're doing at any given moment — and whether there's a known answer to find or a new one to discover.

---

## What I would do differently

1. **Write the Q&A document first.** I spent months building infrastructure before I had a clear articulation of what I was trying to prove. Writing the answers to these 20 questions early would have saved time and focused the engineering.

2. **Run experiments earlier.** The SGA benchmark could have been run in the first week. Instead, I spent weeks optimizing code that the experiment later showed didn't matter for routing quality.

3. **Publish the repo earlier.** The first version of this project was private. Making it public forced me to clean up, document, and think about what others would see. The fear of publishing something unfinished is a great motivator for finishing it.

4. **Spend less time on features, more time on measurements.** Every subsystem I added was another thing to benchmark, document, and maintain. The most valuable parts of the project are the results directory and the reproducibility infrastructure — not the codebase size.

---

## Final thought

This project started as "I want to build a decentralized brain." It's become "I want to find out whether P2P collaborative learning can work over real Internet conditions."

The first statement is a dream. The second is a research question.

The difference between them is the distance this project has traveled. And the reason it traveled that distance is someone who read early versions and said "this is impressive, but here's what's missing" — and was specific enough that I could actually fix each thing.

I don't know whether the answer to the research question will be yes or no. But I know the process for finding out, and I know it generates reproducible evidence either way. That's what I learned how to do.
