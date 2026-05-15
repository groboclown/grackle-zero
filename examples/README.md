# Example Programs

To run these programs, you can use:

```shell
cargo run --example EXAMPLE_NAME
```

and append command-line arguments to the invocation as needed.


## [piped](piped.rs)

Run as:

```shell
cargo run --example piped PATH_TO_EXEC
```

This will run `PATH_TO_EXEC` within the sandbox, passing the stdin, stdout, and stderr through the shell.  You can add CLI arguments to the invocation.  For example, in Unix-like environments, you can run:

```shell
cargo run --example piped $( which echo ) 12345
```

to have it output the combination of your luggage.

And you can run:

```shell
cargo run --example piped $( which ls ) /
```

to see it report that it can't read directories.

On Windows PowerShell, you can run:

```powershell
cargo run --example piped ${env:SYSTEMROOT}\System32\fontview.exe f
```

to reveal that the sandbox prevents user interface elements from showing up.  However, it won't cause the program to generate an error (Windows puts the graphical elements in a non-visible isolate), so you'll need to explicitly kill the program.
