# Project Development Lifecycle

This project is being developed in Australia and therefore uses AU or UK english spellings in all documentation, comments,variable and file names

## Project Guidelines

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
| It is important to use a clean architecture following SOLID principles and clear and unambiguous API for modules, struct, implementations and variants.

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

**IMPORTANT**
| - NEVER commit code or add files to the commit cache unless explicitly asked to do so.
| - If commit is the next logical step, generate a commit message and ask the user to do the actual commit.

## Pre-Work Verification

Before starting any development activity, ensure that:

1. **Repository State:** All changes (excluding unstaged) have been pushed to remote repository
2. **Task Management:** A new YouTrack task has been created and is being actively worked on

This verification ensures clean starting conditions and proper task tracking for all development work.

## Project Workflow Rules

### Issue State Management

- **Backlog:** Issue may be selected to work on
- **Open:** Issue is actively being worked on
- **In Progress:** Implementation is underway following TDD cycle
- **Queued:** TDD cycle is complete and implementation is finished - ready for release
- **Done:** All "Queued" issues are collectively moved to "Done" when the product is built and released on GitHub

### Start of Lifecycle

- When there is no active issue, present a list of available (State in [Backlog, Open]) to the user
- The user will elect the issue to work on next.
- It should be moved state = "In Progress" and the implementation plan generated

### Development Process

- Each YouTrack issue represents a complete feature or infrastructure component
- if an issue is too large a unit of work, suggest to the unit that it be split (see below)
- Follow strict TDD (Red-Green-Refactor) workflow for all development
- Create detailed implementation plans in `devdoc/` directory named `GS-{issue-number}_PLAN.md`
- All verification tests must pass before proceeding to next step
- **IMPORTANT** Request a review from the user once the detailed implementation is marked complete
- Once confirmed and there are no other additional tasks or cleanup to perform:
  - if the TDD cycle and implementation are complete
    1. update `devdoc/README.md` to reflect the current development status
    2. attach the plan to the issue to which it belongs
    3. once successfully attached remove it from the project work area
    4. Move the issue to "Queued" state
- Notes:
  - **IMPORTANT** DO NOT attach plans until they are completed!

#### Splitting Tasks

- Large tasks may need to be split from time to time. When required to do this:
  - Mark the task to be split (the parent) as type = "Feature", state = "Open"
  - Create the new tas or tasks (YouTrack uses its own numbering system)
  - Add a link of type "is subtask of" from the child task to parent
    issue type = "Task", state = "Open"
  - Otherwise do nothing immediately with subtasks until asked to do so.
  - Individual subtasks will be worked on as separate issues
  - Once all subtasks are completed, return to "Start of Lifecycle"

#### Testing

##IMPORTANT** Never mark an issue as complete until you get 100% pass rate on all tests.
Fix all non-working tests. ALWAYS. No excuses.
If tests were passing and start failing after you change code and then it is absolutely a mistake to think that the failures are "unrelated". They most definitely are related, regardless that you initially do not know how or why. FIX THEM.

#### YouTrack Interactions

YouTrack is queried and updated using the `yt` command. Please read YouTrack.md in this directory for details.
One identified issue is that long descriptions or comments cause issues that result in vscode encountering a "PTY" error resulting in failure, a short hang and delay. Shorten long descriptions by making them as concise as possible.
