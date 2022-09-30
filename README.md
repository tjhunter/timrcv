# timrcv - ranked choice voting made easy and fast

A battle-tested command line vote tabulator for ranked-choice voting algorithms, also known as alternative vote (UK), single transferable vote (Australia) and instant-runoff voting.

`timrcv` quickly and simply calculates election results from recorded votes. It implements multiple variants of the instant-runoff algorithm (see [Configuration](#configuration) section). It is fast, uses little resources, and it is available for Windows, MacOS and Linux (see [Download](#download) section). It can read voting data from all the popular sources (Microsoft Forms, Google Forms, Qualtrics, CSV) as well as all the commercial vendors (ES&S, Dominion, NIST CDF). `timrcv` produces results that can be quickly visualized in all sorts of formats with [RCVis](www.rcvis.com).

If you want to use `timrcv`, look at the [Download](#download) instructions and the [Quick start](#quickstart) to get started.

`timrcv` is a clean room reimplementation of [RCVTab](https://www.rcvresources.org/rctab) in the Rust programing language. It supports the same configuration files and inputs, with the advantage of being significantly faster (8x-50x observed speedups). *Please note that unlike RCVTab, timrcv has not been audited. Consider using RCVTab instead for official tabulation needs*.

## Download

Download the latest release from the [releases page](https://github.com/tjhunter/timrcv/releases). Pre-compiled versions are available for Windows, MacOS and Linux.

## License

`timrcv` is licensed under the Apache 2.0 License.

## Quick start

To get started, let us say that you have a file with the following records of votes ([example.csv](https://github.com/tjhunter/timrcv/raw/main/tests/csv_simple_2/example.csv)). Each line corresponds to a vote, and A,B,C and D are the candidates:

```
A,B,,D
A,C,B,
B,A,D,C
B,C,A,D
C,A,B,D
D,B,A,C
```
Each line is a recorded vote. The first line `A,B,,D` says that this voter preferred candidate A over everyone else (his/her first choice), followed by B as a second choice and finally D as a last choice.

Running a vote with the default options is simply:

```
timrcv --input example.csv
```

Output:

```
[ INFO  ranked_voting] run_voting_stats: Processing 6 votes
[ INFO  ranked_voting] Processing 6 aggregated votes
[ INFO  ranked_voting] Candidate: 1: A
[ INFO  ranked_voting] Candidate: 2: B
[ INFO  ranked_voting] Candidate: 3: C
[ INFO  ranked_voting] Candidate: 4: D
[ INFO  ranked_voting] Round 1 (winning threshold: 4)
[ INFO  ranked_voting]       2 B -> running
[ INFO  ranked_voting]       2 A -> running
[ INFO  ranked_voting]       1 C -> running
[ INFO  ranked_voting]       1 D -> eliminated:1 -> B, 
[ INFO  ranked_voting] Round 2 (winning threshold: 4)
[ INFO  ranked_voting]       3 B -> running
[ INFO  ranked_voting]       2 A -> running
[ INFO  ranked_voting]       1 C -> eliminated:1 -> A, 
[ INFO  ranked_voting] Round 3 (winning threshold: 4)
[ INFO  ranked_voting]       3 A -> running
[ INFO  ranked_voting]       3 B -> eliminated:3 -> A, 
[ INFO  ranked_voting] Round 4 (winning threshold: 4)
[ INFO  ranked_voting]       6 A -> elected
[ INFO  ranked_voting]         undeclared candidates: 
```

`timrcv` supports many options (input and output formats, validation of the candidates, configuration of the tabulating process, ...). Look at the Configuration section below for more details.

## Formats

The following formats are supported:
* `ess` ES&S company
* `dominion` Dominion company
* `cdf` NIST CDF

### `ess`

Votes recorded in the ES&S format (Excel spreadsheet).

### `dominion`

Votes recorded in the format from the Dominion company.

### `cdf`

Votes recorded in the Common Data Format from NIST.

Notes:
- only the JSON notation is currently supported (not the XML)
- only one election is supported

### `msforms`

Results from Microsoft Forms when using the ranking widget. See the example in the `tests` directory.

### `msforms_likert`

Results from Microsoft Forms when using the 'Likert' input. See the example in the `tests` directory. Your form is expected to be formatted as followed:


|             | choice 1 | choice 2 | ... |
|-------------|----------|----------|-----|
| candidate A |          | x        |     |
| candidate B | x        |          |     |
| ...         |          |          |     |

In this example, this vote would mark `candidate B` as the first choice and then `candidate A` as a second choice.

In this case, both the names of the choices and of the candidates are mandatory. See the example `msforms_likert` for an example of a configuration file.

### `msforms_likert_transpose`

Results from Microsoft Forms when using the 'Likert' input with the candidates in the first row. See the example in the `tests` directory. Your form is expected to be formatted as followed:

|               | candidate A | candidate B | ... |
|---------------|-------------|-------------|-----|
| first choice  |             | x           |     |
| second choice | x           |             |     |
| ...           |             |             |     |

In this example, this vote would mark `candidate B` as the first choice and then `candidate A` as a second choice.

In this case, both the names of the choices and of the candidates are mandatory. See the example `msforms_likert_transpose` for an example of a configuration file.

### csv

Simple CSV reader. Each column (in order) is considered to be a choice. The name of the choice in the header is not significant.

```
id,count,choice 1,choice 2,choice 3,choice 4
id1,20,A,B,C,D
id2,20,A,C,B,D
```

The `id` and `count` columns are optional. Headers in the first row is optional.

### csv_likert

Simple CSV reader sorted by candidates. This format is also created by Qualtrics polls. The file is expected to look as follows:

```
id,count,A,B,C,D
id1,20,1,2,3,
id2,20,1,3,2,4
```

The `id` and `count` columns are optional. The candidate names must all be a column and defined in the first row of the CSV file. The numbers below are the ranks of this candidate for each ballot (or empty if this candidate was not ranked).

## Configuration

`timrcv` comes with sensible defaults but users may want to apply specific rules (for example, how to treat blank choices). The program accepts a configuration file in JSON that follows the specification of the [RCVTab program]()

See the [complete documentation](https://github.com/BrightSpots/rcv/blob/develop/config_file_documentation.txt) for more details. Note that not all options are supported. Contributions are welcome in this area.

## Contribute

Contributions are welcome. Contributions for documentation are always appreciated. Contributions that provide examples of past elections are especially welcome, in particular for non-US political systems that also use instant-runoff voting (Australia, New Zealand for example).

In general, other algorithms or variations of ranked-choice voting algorithms will be considered if they apply to real-life election cases. If you have a datasets from a past election that you cannot process with `timrcv`, please open a bug report to open a discussion.

