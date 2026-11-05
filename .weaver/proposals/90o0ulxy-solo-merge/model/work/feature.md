# Work

The work that comes out of the Model: Tasks the agent carries out, Drifts where the code and the Model or the Requirements disagree, and the History of what was done. The Board shows what waits for the developer's decision (open Proposals and Drifts) and the Tasks still to finish; finished work leaves the Board and is told in History.

A Task is a unit of work for the agent. The developer runs it, or runs all the Tasks in To do, and the agent carries them out in a Run; he never moves them by hand. A Run works on several Tasks at once with sub-agents, never two that concern the same thing, and while it works omp's advisor checks that the code the agent reads or writes follows the Task, the Model and the Requirements: a difference in the Task's own work is corrected at once, a difference in code that was already there becomes a Drift. Only one Run works at a time in a project, and a Run and a Resolve exclude each other.

A Drift is a difference the developer must decide: fix the code, accept a Proposal that changes the Model, or let it be. No Drift is decided, resolved or carried out by itself. While a Drift is open, the Tasks that concern the same thing wait.

Two things concern the same thing when they name the same element of the Model, or one names an element inside the other; a Requirement section concerns the element it is titled with, and a General section only concerns itself. The same rule decides blocking, which Tasks a Run may carry out together, and which Drifts touch a Task.

A scan compares the Model and the Requirements with the code: the developer asks for it on the whole System, or Weaver starts it by itself once a merge in Solo mode is concluded, telling the agent which files the merge brought; the agent compares every Machine, each one with a sub-agent, filing Tasks for what is missing and Drifts for what disagrees. A Machine with no differences files nothing.
