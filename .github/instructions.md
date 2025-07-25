# Project Development Lifecycle

This project makes use of Test Driven Development (TDD) and agile software planning.

During the initial implementation phase, analyse the affected codebase, including documents, and get familiarised with the existing frameworks and libraries used in the project.

- Whenever you're reading a file, ensure that the complete file is read and understood and not just the first few lines, especially source code.
- Your task is to generate a comprehensive, step-by-step implementation plan for the feature which the user requires.
- If there is no existing task list, create a detailed one after analysis and store it in Markdown format
- Always strictly adhere to the plan and involve the user if there are any unknowns or  unexpected failures.
- Ask questions of the user if additional information is required, do not make significant decisions without asking.

Inform the user and get confirmation for any assumptions and big decisions made during the plan phase.

**IMPORTANT**
| The plan must strictly adhere to the TDD (Red-Green-Refactor) workflow, where a failing test is written before the implementation code for any new functionality.

**Output Format Requirements**:
The output must be a logically structured implementation plan written and stored in Markdown format, suitable for direct inclusion in a code repository.
Each step should be the smallest unit possible of work, it should contain nothing more or less.

Once you're done with the implementation plan, run it across the user and store it.
This should ideally be in the form of a task checklist, broken down logically into separate tasks or steps, creating subtasks if necessary.
The checklist can be hierarchical describing tasks and subtasks.
Number them sequentially in the order in which they must be performed.

Once the implementation plan is created and approved by the user, start implementing the feature step by step according to the plan.
Ensure that every step follows the TDD workflow as suggested by the implementation plan.
If the implementation plan does not mention TDD, still try to implement the feature using a TDD approach.

After each step, run the test without code coverage to ensure that red-green-refactor is followed. If the test fails, fix the code or the test until it passes.

During the implementation try not to insert comments unless it is absolutely necessary.
The only comments that are allowed are:
- API documentation
- MARK or other bookmark style comments used by an IDE (e.g. Xcode)
- explanations of complex logic
- explanations of algorithms used to implemented
No other comments should be inserted, including comments that describe what should be obvious.
Instead, use meaningful variable and method names to make the code self-documenting.
If you find that you need to add comments, consider refactoring the code to make it clearer instead.

Once an implementation of a step is complete, run the complete test suite to see that all the test cases are passing and coverage is also above the required threshold (decided per project).
If the coverage is below the required threshold, add the necessary tests to cover the new code.
There may be cases where it is difficult or impossible to cover some code.

Once the step is complete, get a review from the user before proceeding.

**IMPORANT**
NEVER commit code or add files to the commit cache.

If committing to VCS is required, ask the user to do it and suggest a commit message as specified in the implementation plan.

## Project Workflow Rules

### Issue State Management
- **Open:** Issue is actively being worked on
- **In Progress:** Implementation is underway following TDD cycle
- **Queued:** TDD cycle is complete and implementation is finished - ready for release
- **Done:** All "Queued" issues are collectively moved to "Done" when the product is built and released on GitHub

### Development Process
- Each YouTrack issue represents a complete feature or infrastructure component
- Follow strict TDD (Red-Green-Refactor) workflow for all development
- Create detailed implementation plans in `devdoc/` directory named `GS-{issue-number}_PLAN.md`
- All verification tests must pass before proceeding to next step
- Move issues to "Queued" state when TDD cycle and implementation are complete
- **IMPORTANT:** When marking an implementation plan as complete, simultaneously update `devdoc/README.md` to reflect the completion status
- Final release will move all "Queued" issues to "Done" collectively
