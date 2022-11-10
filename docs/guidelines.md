# Guidelines

## Tests

To understand why there should be any testing guidelines, the most important question
to answer is: why write tests at all? What is the goal of tests?

To list a few benefits of automated testing:

- fewer bugs,
- reading tests can help understand the code, if they are written in a careful way,
- reading tests can help understand how to use the program under test,
- but most importantly, tests give confidence that refactoring code did not introduce any new bugs.

Perhaps unintuitively, the last point is the most important one. This is because the natural
direction of any software project is rot. The only way to avoid rot is to refactor.

When implementing a feature, the developer has a choice. He can introduce the feature
in a way that leads to the smallest PR, but feels a little awkward for the reader or unnatural
in some other way. This is justified by the need for development speed. As more and more features
are added this way, the codebase becomes harder and harder to read. Development speed originally
praised now gradually decays. Eventually the only solution
is a large set of refactorings that feel akin to rewriting all or parts of the project.

If the developer instead accepts that refactoring and implementing features must go hand in hand,
he will refactor along with each feature he adds. He can refactor first and then implement the feature,
or implement the feature and then refactor, but either way the goal is the same: implement the
feature in a way that isn't necessary the smallest set of PRs, but _is_ the best experience for
the future reader and keeps a coherent architecture of the code.

_The only way to go fast is to go well._

Here are some drawbacks of testing:

- it is time-consuming,
- it introduces a maintenance burden, since whenever some code is changed some tests also
have to be changed.

There isn't much to be done about the first point: all software development takes time, and
manual testing takes time too. The second point is more interesting: every person reading this
was in a place where they introduced some complex refactoring to some piece of code, and then
had to change tons of tests to get the code to compile and the test suite to pass.

This situation is extremely common and symptomatic. In fact, if the goal of testing is to
allow refactoring with confidence, we have a paradox: you have to change tests whenever
you refactor, so how can you have confidence that you didn't make a mistake now when you
changed the tests? There is no longer any confidence to gain from the test suite. Such a
test suite is commonly referred to as _brittle_.

The way to save ourselves from both the maintenance burden and the paradox above is to write tests
_at the highest possible level of abstraction_. This is why I intentionally use the
term "automated tests" and not "unit tests": the industry can not and never will agree
on what the "unit" of a "unit test" is supposed to be. People often take "unit" to
mean "module". This is a terrible mistake, as it will lead you right into the issues
described above.

Instead, the most effective interface to test is the _highest_ interface exposed by
the program. If you can effectively test by calling main with CLI arguments, that's
really good. If you can run the HTTP server locally and send API requests to it,
that's also good. If you can't or don't want to do either of those two, the next
best thing is to test only those interfaces _used by main_.

My ideal state for tests is to only break tests when the public interface of the
program itself changes, and in no other situation.

### Mocking

Not getting into the minutia of mocks vs. stubs vs. substitutes, this doesn't matter.

Too much mocking is generally indicative of being in the situation I described above -
the "unit" to test was decided to be a single type, so every other type that the type
under test depends upon is being mocked. Maintenance burden and brittle tests abound.

The proper approach to mocking is to only mock external elements of the system. For example,
the price estimation algorithm is not an external element. However, the HTTP servers that
are contacted by the price estimation algorithm _are_ completely external to our system.

The reason is simple: to truly test the logic of your program, you should put under test
as much logic as possible. If you only test bits of logic, you're not testing how they work together.
So core logic (more precise definition of this later) should never be mocked out.

Finally, there is the question of whether or not the DB should be mocked.
Strictly speaking, the DB is external to the system. However, I would advocate for
_not_ mocking out the DB due to the following reasons:

1. With `docker-compose`, it's very easy to set up a local environment for unit tests.
So it's not unreasonable to expect the developers to be able to conjure up the DB with
a single `docker-compose up` command.
2. The DB is _extremely_ integral and common to the application. Testing it is very beneficial,
as it ensures not only that the SQL queries are correct, but also that the SQL queries interact
correctly with the rest of the code.

The main drawback of not mocking out the DB is that tests will be a bit slower, as the DB has to be
cleared/queried/updated.

I'd like to hear what the team thinks about mocking or not mocking the DB. If we decide to mock
the DB, a good pattern to apply in this case is the Repository pattern, and I will describe it
here in more detail.

## Clean Architecture

Ideas here are based on works by Robert C. Martin and others. The most influential
book in this sphere is [Clean Architecture](https://www.amazon.com/Clean-Code-Handbook-Software-Craftsmanship/dp/0132350882).
For people interested in software engineering, see [here](https://gist.github.com/ennmichael/372ad641a8ea50cc29d6fa0a18c5ba10).

### Core Logic

In the enterprise world, this is more commonly referred to as domain logic. The idea is
the following: almost every application today has common elements that essentially come
down to boilerplate. Things like database access, metrics, the API layer, etc. Generally
all of these things can be answered once and for all and implemented by convention down
the line.

The value that your software project brings to the business is not in these repeated
elements of boilerplate. Instead the value resides at the core logic of your application
that solves the particular problem domain of your business. For example, every web application
talks to a database, exposes an API or a web UI, has some metrics, etc. And so does
cowswap.fi. But cowswap is different than all these other applicaitons because it solves
the problem of finding the best exchange prices on the blockchain in a unique way.
This is the core logic and the most valuable part of our application: the logic that
describes how this problem is solved.

Considering all code in your project, this part of the code is the most important. It needs to
be isolated, decoupled, well-documented, etc. This is the code which a newcomer needs to read
to understand how your application works and how the problem is solved. Nobody wants to be
bogged down in SQL query details when they're trying to learn how price estimation works!

The idea comes down to a _separation of concerns_. The DB details, the metrics details,
etc. are all a different concern than the core logic. Then furthermore, the core logic
has different areas of concern as well (sometimes referred to as bounded contexts, but
this is less important): order logic, quoting logic, settlement logic, etc.

This means that every other interface should be tailored to the use case of the core logic.
For example, all of the concepts in your project should be defined by the core logic. The
`Order` type should be defined by the code logic. The DB layer needs to store these `Orders`.
The DB layer will map these to some other type (commonly referred to as data transfer object, DTO),
but the core logic should know nothing about this mapping. Furthermore, it should know as little
as possible about _how_ the storage happens. The core logic should state what it wants to have happen,
and the DB layer should ensure that it does happen.

For example, let's imagine an interface of a DB module. This is what you shouldn't do:
```rs
// db.rs

pub struct LimitOrderRowUpdate;

pub struct Db;

pub fn update_limit_order_row(db: &Db, update: &LimitOrderRowUpdate);

// core.rs

pub struct LimitOrder;

fn map_limit_order_to_row_update(order: &LimitOrder) -> db::LimitOrderRowUpdate;

pub fn process_limit_order(order: &LimitOrder) {
  // ...
  db::update_limit_order_row(map_limit_order_to_row_update(order));
  // ...
}
```

Instead, the `core` module should determine the interface of the `db` module:

```rs
// data.rs - note that the name is more generic than "db"

// Note that this is no longer exported, but only used internally by this module
struct LimitOrderRowUpdate;

pub struct Store;

// Note that this module uses the type defined by the core module
pub fn save_order(store: &Store, order: &core::LimitOrder);

// core.rs

pub struct LimitOrder;

pub fn process_limit_order(order: &LimitOrder) {
  // ...
  // No details related to the DB, updates, or rows.
  store::save_order(order);
  // ...
}
```

The principle above is often called the dependency inversion principle. If you look up
this term online, you will get a lot of bad hits that explain it wrong. The Clean Architecture
book, which contains the original definition, explains the point a lot more clearly.

The point is that the interface exposed by the `data` module should be "owned" by the
`core` module. The `core` module determines the interface for data, not the other way around.

This can be codified using traits. For example:

```rs
// core.rs

pub trait Store {
  fn save_order(&self, order: &LimitOrder);
}

pub struct LimitOrder;

pub fn process_limit_order(store: impl Store, order: &LimitOrder) {
  // ...
  store.save_order(order);
  // ...
}

// data.rs

struct Store;

impl core::Store for Store {}

pub fn create_store() -> impl core::Store {
  Store
}
```

Now the `data` module depends on the `core` module and the core module does not even have
a dependency on the `data` module at all. Furthermore, the interface that the `core` module
needs is defined in the `Store` trait, directly in the `core` module.

This idea of using traits is very neat and practical if you want to mock. If you don't want
to mock using modules is more pragmatic, but always keep in mind the understanding that `core`
dictates the interface that `data` exposes.

To summarize, the above approach of separating core logic allows you to put an accent on
the code that is most important and leave boilerplate decisions up to conventions that
the team has already agreed on.

### Slicing

I will use part of the proposed architecture from the [architecture document](./architecture.md)
as an example.

Let's imagine we have the following architectural concerns: some persistent (database) storage,
some metrics, and communicating with some external servers (e.g. the 1inch API).

Option 1:

- `cowswap`
    - `solution`
        - `core` - contains the core logic
        - `data` - contains the SQL queries, or alternatively implements some `Store` traits from `core`
        - `metrics` - contains the details for metrics, or alternatively implements some `Metrics` traits from `core`
        - `external` - implements external traits from `core`, e.g. `trait OneInch`, `trait Paraswap`, or similar.
    - `quote`
        - `core` - contains the core logic
        - `data` - contains the SQL queries, or alternatively implements some `Store` traits from `core`
        - `metrics` - contains the details for metrics, or alternatively implements some `Metrics` traits from `core`
        - `external` - implements external traits from `core`, e.g. `trait EthBlockchain` for scanning transactions.

Option 2:

- `cowswap`
    - `core` - contains all core logic
        - `solution`
        - `quote`
    - `data` - contains all SQL queries, or alternatively implements all `Store` traits from `core` modules
        - `solution`
        - `quote`
        - note: the above two could be a single module if there isn't too much code
    - `metrics` - contains all details for metrics, or alternatively implements all `Metrics` traits from `core` modules
        - `solution`
        - `quote`
        - note: the above two could be a single module if there isn't too much code
    - `external` - implements all external traits from `core`
        - `solution`
        - `quote`
        - note: the above two could be a single module if there isn't too much code

Of course other than `cowswap` there could be other top-level modules. E.g. a top-level `data` module
for defining some common DB types maybe.

### The Conventions

I would like the team to answer the following questions:

- Do we want to mock the db?
- How do we want to slice the code, option 1 or option 2?
