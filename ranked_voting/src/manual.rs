/*!

This is the long-form manual for `ranked_voting` and `timrcv`.

## Input formats

The following formats are supported:
* `ess` ES&S company
* `dominion` Dominion company
* `cdf` NIST CDF
* `csv`, `csv_likert` Comma Separated Values in various flavours
* `msforms`, `msforms_likert`, `msforms_likert_transpose` Input from Microsoft Forms and Google Forms products.

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

Results from Microsoft Forms when using the ranking widget.
The input file is expected to be in Excel (.xlsx) format.
See the example in the `tests` directory.

### `msforms_likert`

Results from Microsoft Forms when using the 'Likert' input. It is also compatible with
Google Forms when candidates are the rows and choices are the columns.
The input file is expected to be in Excel (.xlsx) format.

See the example in the `tests` directory. Your form is expected to be formatted as followed:


|             | choice 1 | choice 2 | ... |
|-------------|----------|----------|-----|
| candidate A |          | x        |     |
| candidate B | x        |          |     |
| ...         |          |          |     |

In this example, this vote would mark `candidate B` as the first choice and then `candidate A` as a second choice.

In this case, both the names of the choices and of the candidates are mandatory. See the example `msforms_likert` for an example of a configuration file.

### `msforms_likert_transpose`

Results from Microsoft Forms when using the 'Likert' input with the candidates in the first row.
It is also compatible with Google Forms when the rows are the choices and the columns are
the candidates. The input file is expected to be in Excel (.xlsx) format.
See the example in the `tests` directory. Your form is expected to be formatted as followed:

|               | candidate A | candidate B | ... |
|---------------|-------------|-------------|-----|
| first choice  |             | x           |     |
| second choice | x           |             |     |
| ...           |             |             |     |

In this example, this vote would mark `candidate B` as the first choice and then `candidate A` as a second choice.

In this case, both the names of the choices and of the candidates are mandatory. See the example `msforms_likert_transpose` for an example of a configuration file.

### csv

Simple CSV reader. Each column (in order) is considered to be a choice. The name of the choice in the header is not significant.

```text
id,count,choice 1,choice 2,choice 3,choice 4
id1,20,A,B,C,D
id2,20,A,C,B,D
```

The `id` and `count` columns are optional. Headers in the first row is optional.
See the [Configuration section](#configuration) on controling the optional rows and columns.

### csv_likert

Simple CSV reader sorted by candidates. This format is also created by Qualtrics polls. The file is expected to look as follows:

```text
id,count,A,B,C,D
id1,20,1,2,3,
id2,20,1,3,2,4
```

The `id` and `count` columns are optional. The candidate names must all be a column and defined in the first row of the CSV file. The numbers below are the ranks of this candidate for each ballot (or empty if this candidate was not ranked).

## Configuration

`timrcv` comes with sensible defaults but users may want to apply specific rules
(for example, how to treat blank choices). The program accepts a configuration file in JSON that follows the specification of the [RCVTab program]()

See the [complete documentation](https://github.com/BrightSpots/rcv/blob/develop/config_file_documentation.txt) for more details.
 Note that not all options are supported and that some options have been added to better control the use of CSV.
 Contributions are welcome in this area.

The deviations from the specification of RCVTab are documented below.

> Note: this documenation is incomplete for now.

Deviations for FileSource:
 - added `count_column_index` (string or number, optional): the location of the column that
 indicates the counts. If not provided, every vote will be assigned a count of 1.

 - added `excel_worksheet_name` (string, optional): for Excel-based inputs, the name of
 the worksheet in Excel.

 - added `choices` (array of strings, optional): The list of labels for the choices. For example, if
   the list is `["First choice", "Second choice"]`, then seeing `First choice` will be
   intepreted as choice #1, and so on.


Deviations for OutputSettings:
- removed `generateCdfJson`: feature not supported
- removed `tabulateByPrecinct`: feature not supported

 */
