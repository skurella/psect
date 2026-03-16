# psect: a probabilistic alternative to git bisect

Bisection, or binary search, is an excellent tool for finding deterministic regressions. However, it falls short when the regression introduces flakiness.

Repeating the test N times on each revision is wasteful:

- The more times you run it on each revision, the slower and more expensive the bisection becomes.
- If you don't run it a sufficient number of times, you're likely to converge on the wrong revision.

`psect` is a bisection tool that treats flakiness as a first-class concept. It deals with three probability distributions:

- test pass rate before and after the regression
- likelihood for each of the revisions to have introduced the regression.

The next revision to test is selected such that it is expected to have the lowest entropy, i.e. yield the highest information gain.

## Installation

```sh
cargo install git-psect
```

If the build fails with an OpenSSL error (common on systems with a missing or incompatible system OpenSSL), compile with the vendored feature to build OpenSSL from source:

```sh
cargo install git-psect --features vendored
```

To install from source:

```sh
cargo install --path crates/git-psect
```

## Usage

Start a session, mark bounds, then either drive it manually or let `run` handle the loop:

```sh
git psect start
git psect old <rev>   # before the suspected regression, but may still be flaky
git psect new <rev>   # a revision where the test fails more frequently
```

Both `old` and `new` default to `HEAD` if `<rev>` is omitted. Any refspec accepted by `git rev-parse` works — `@{1-week-ago}`, `:/'commit message'`, `main`, etc.

`psect` checks out the most promising commit to test. Then either run the test manually and mark the result:

```sh
git psect pass        # record that the test passed at HEAD
git psect fail        # record that the test failed at HEAD
```

Or let `psect` drive the loop:

```sh
git psect run ./test.sh
```

`run` checks out revisions, runs the script, records outcomes, and stops when it reaches the default 95% confidence threshold. You can override it with e.g. `--confidence 0.99`.

### Priors

`psect` models the test as a Bernoulli process. The default priors assume the test passes 95% of the time on pre-regression revisions and 50% of the time on post-regression revisions. If you have better estimates:

```sh
git psect set-prior old 0.99   # test almost never flakes on good revisions
git psect set-prior new 0.1    # test almost always fails after the regression
```

Tighter priors require fewer samples to converge. If your priors are too optimistic, the reported confidence will be overly optimistic as well.

### Inspecting state

```sh
git psect state             # print the path to the state.toml file
taplo get -f $(git psect state) -o json "samples"
git psect reset             # clear the session
```

State lives at `$GIT_DIR/psect/state.toml`, so each worktree gets its own independent session.

### Example

Notice how the utility keeps rerunning the test on revision `f`, just before `break test.sh`. This is because it's expecting post-regression commits to only fail 50% of the time, and rerunning on `f` is how it gains confidence in its diagnosis.

```text
% git log --oneline main
11cc33a (HEAD -> main) m
839198c l
e5f545a k
2968124 j
ee230f7 i
5abdb90 h
2325326 g
09b5262 break test.sh
548bdef f
15969fd e
6177e1a d
bc5918e c
2893fd7 b
9bda9cf a
d81b7c4 add test.sh

% git psect start
Session initialized.
Waiting for reference pre-regression and post-regression revisions.
Mark them with 'git psect old <rev>' and 'git psect new <rev>'.

% git psect new main
Marked 11cc33a420 as new.
Expecting the test to pass 50% of the time after the regression.
Change with 'git psect set-prior new <rate>'.
Now mark a reference pre-regression revision with 'git psect old <rev>'.

% git psect old :/add
Marked d81b7c493a as old.
Expecting the test to pass 95% of the time before the regression.
Change with 'git psect set-prior old <rate>'.
Checking out 548bdefc33 "f".
Now either:
- run your test and call 'git psect pass' or 'git psect fail', or
- use 'git psect run <test>' to run on autopilot.

% git psect run ./test.sh
548bdefc33: running test...
548bdefc33: test passed
2325326f36: checking out "g"
2325326f36: running test...
2325326f36: test failed
15969fd531: checking out "e"
15969fd531: running test...
15969fd531: test passed
548bdefc33: checking out "f"
548bdefc33: running test...
548bdefc33: test passed
09b52626a7: checking out "break test.sh"
09b52626a7: running test...
09b52626a7: test failed
548bdefc33: checking out "f"
548bdefc33: running test...
548bdefc33: test passed
Current best guess: 09b52626a7 "break test.sh" (59.2% confidence).
548bdefc33: checking out "f"
548bdefc33: running test...
548bdefc33: test passed
Current best guess: 09b52626a7 "break test.sh" (69.5% confidence).
548bdefc33: checking out "f"
548bdefc33: running test...
548bdefc33: test passed
Current best guess: 09b52626a7 "break test.sh" (76.5% confidence).
548bdefc33: checking out "f"
548bdefc33: running test...
548bdefc33: test passed
Current best guess: 09b52626a7 "break test.sh" (80.8% confidence).
09b52626a7: checking out "break test.sh"
09b52626a7: running test...
09b52626a7: test failed
Current best guess: 09b52626a7 "break test.sh" (91.5% confidence).
548bdefc33: checking out "f"
548bdefc33: running test...
548bdefc33: test passed
Current best guess: 09b52626a7 "break test.sh" (94.6% confidence).
548bdefc33: checking out "f"
548bdefc33: running test...
548bdefc33: test passed
96.4% chance the regression was introduced in 09b52626a7: break test.sh.
Run 'git psect reset' to clear the session or continue running tests to increase confidence.
```

### Future work

- handle arbitrary revision DAGs
- accept normal distributions of pre- and post-regression test outcomes
  - this will enable the benefits of `psect` for finding performance regressions
- accept Beta distributions of pre- and post-regression test outcomes
  - Beta distributions can be updated with test outcomes, making the bisection less sensitive to the priors
- handle choosing multiple simultaneous revisions to test
  - this isn't work-preserving but is common practice in large companies to accelerate finding the result
- factor in build time by weighing expected information gain
  - i.e. $\arg\max_{k} \frac{H(R) - H(R \mid c_k)}{t_\text{build}(i) + t_\text{test}}$, where $t_\text{build}$ is 0 for the current revision
- performance

## Maths

Let's build some intuitions first.

Git-bisect chooses the median commit between the latest known good and the earliest known bad revisions. Below, we'll develop a justification for this as a strategy which minimizes the expected entropy. Then, relaxing the assumptions on the distribution of test outcomes will yield formulae that are implemented in this crate.

### Maximizing the information gain

Let's assume I need to bisect a branch containing 8 commits $c_1..c_8$, where a test on the fork point with `main` $c_0$ always passes and always fails on HEAD $c_8$. We also assume each commit is equally likely to have caused a regression, and that there exists a commit $c_R$ such that:

$$
\begin{aligned}
    test(c_k) &= pass\text{, } && \text{for } k \lt R \\
    test(c_k) &= fail\text{, } && \text{for } k \ge R \\
\end{aligned}
$$

This yields an entropy of $3$. If we were to run the test on $c_4$, we'd expect it to fail if $c_R \in \{c_1, c_2, c_3, c_4\}$ and pass if $c_R \in \{c_5, c_6, c_7, c_8\}$. With each outcome, we'll only need to choose between 4 commits. The expected entropy is then:

$$
\mathbb{E}(H(R')) = \frac{4}{8} \times \log_2(4) + \frac{4}{8} \times \log_2(4) = 2
$$

Therefore, choosing $c_4$ yields an *expected information gain* of 1 shannon.

What if we were instead to run the test on $c_3$?

$$
\begin{aligned}
    \mathbb{E}(H(R'))
    &= P(c_R \in \{c_1..c_3\}) \times \log_2(3) + P(c_R \in \{c_4..c_8\}) \times \log_2(5) \\
    &= \frac{3}{8} \times \log_2(3) + \frac{5}{8} \times \log_2(5) \\
    &\approx 2.05
\end{aligned}
$$

So we expect to have more work still left to do after testing $c_3$ than $c_4$. In fact, entropy in a *concave* function. This means that, for symmetric probability distributions, we should aim to divide the decision space into equally likely halves. Therefore, choosing the middle commit is the optimal choice in this situation.

### Modelling a flaky test

Let us now reformulate the flaky test as a function sampling from one of two distributions:

$$
\begin{aligned}
    t(c_k) &\sim \text{Bernoulli}(p_{old}) \text{, } && \text{for } k \lt R \\
    t(c_k) &\sim \text{Bernoulli}(p_{new}) \text{, } && \text{for } k \ge R \\
\end{aligned}
$$

$R$ is the index of the commit which introduces the regression. The purpose of running the bisection is to find it with sufficient confidence while minimizing the number of iterations.

$p_{old}$ and $p_{new}$ are probabilities of the test passing on commits before and after the regression, respectively. We don't necessarily know those, too. However, we might take some prior assumptions, for example:

- $p_{old}=1$, i.e. we're confident that, at least on the baseline commit, the test always passes.
- $p_{new}=\frac{2}{3}$, because we saw it fail 1 out of 3 times we ran it locally.

### Prior distribution of $R$

In the initial state, we don't have any information about the relative likelihood of commits $c_{1 \dots N}$, so we assume that they are equally likely:

$$
P(R=k) = \frac{1}{N}
$$

This leads to an initial entropy of $H(R)=\log_2(N)$.

### Expected result of testing $c_k$

Following our assumptions, running the test on $c_k$ draws from one of two distributions.

$$
\begin{aligned}
    P(t(c_k)=1 \mid k \lt R) &= p_{old} \\
    P(t(c_k)=1 \mid k \ge R) &= p_{new} \\
\end{aligned}
$$

We also know that:

$$
\begin{aligned}
    P(k \lt R) &= \sum_{i \in (1 .. k-1)} p(c_i) \\
    P(k \ge R) &= \sum_{i \in (k .. N)} p(c_i) \\
\end{aligned}
$$

Therefore, the probability of a given outcome can be calculated using the law of total probability:

$$
\begin{aligned}
    P(t(c_k)=1)
    &= \sum_{i \in (1..N)} P(t(c_k)=1 \mid R=i) \cdot P(R=i) \\
    &= P(t(c_k)=1 \mid k \lt R) \cdot P(k \lt R) + P(t(c_k)=1 \mid k \ge R) \cdot P(k \ge R) \\
    &= p_{old} \sum_{i \in (1..k-1)} p(c_i) + p_{new} \sum_{i \in (k..N)} p(c_i)
\end{aligned}
$$

### Posterior distribution of $R$

If we did actually run the test and saw it pass or fail, the new knowledge would enrich our understanding of how likely each commit is to have introduced the regression, i.e. the posterior distribution of $R$. Let's apply the Bayes principle, assuming the test passed:

$$
\begin{aligned}
    P(R=i \mid t(c_k)=1)
    &= \frac{P(R=i, t(c_k)=1)}{P(t(c_k)=1)} \\
    &= P(t(c_k)=1 \mid R=i) \frac{P(R=i)}{P(t(c_k)=1)} \\
    &\propto P(R=i) \cdot P(t(c_k)=1 \mid R=i) \\
    &= P(R=i) \cdot
        \begin{cases}
            p_{old} & \text{if } k \lt i \\
            p_{new} & \text{if } k \ge i \\
        \end{cases}
\end{aligned}
$$

And similarly, if the test fails:

$$
P(R=i \mid t(c_k)=0) \propto P(R=i) \cdot
\begin{cases}
    1 - p_{old} & \text{if } k \lt i \\
    1 - p_{new} & \text{if } k \ge i \\
\end{cases}
$$

Note that this is fully iterative. Each new test outcome mutates the distribution from $P(R=i \mid E) \to P(R=i \mid E\prime)$ where $E$ is the prior evidence and $E\prime$ is that same evidence as well as the new test outcome. To prove this, simply append $E$ as a condition to every probability distribution above.

### Expected information gain from testing $c_k$

We still need a way to choose a commit $c_k$ to test next. However, we now have the means of calculating the entropy of $R$ given zero or more test outcomes. Note that we don't actually need to run a test to calculate what the resulting entropy *would* be if it were to yield a given result. Therefore, we can get the *expected* entropy after choosing $c_k$ with:

$$
\mathbb{E}(H(R \mid t(c_k))) = P(t(c_k)=1) \cdot H(R \mid t(c_k)=1) + P(t(c_k)=0) \cdot H(R \mid t(c_k)=0)
$$

Finally, we choose the commit $c_k$ which maximizes the information gain, i.e. minimizes the posterior entropy:

$$
\begin{aligned}
    k\prime
    &= \arg\max_{k} H(R) - H(R \mid t(c_k)) \\
    &= \arg\min_{k} H(R \mid t(c_k))
\end{aligned}
$$
