# Windows Implementation Details and Notes

## End-User Notes

***Information for end-users on Windows computers.***

The implementation uses Windows AppContainer technology to help isolate the restricted application's shared data (such as temporary files and registry entries), and to limit the application's capabilities.  Unfortunately, Windows manages these constructed AppContainer profiles with the expectation that they live for the application's installation lifetime, not for the duration of execution.  That's partly because of how heavyweight these are.

That's a lot of words to say that, in the case the program performs a hard stop, the AppContainer profile created for the execution won't be cleaned up.  This can lead to leaked resources sitting on your computer that you may not want.

If you *know* that none of these applications are running, then it *should* be safe to run the [included PowerShell script](cleanup-appcontainers.ps1) to clean up these extra AppContainer profiles.


## Developer Notes

***Information for developers using this library for Windows programs.***



## Implementation Details

***Information for developers of this library, or for users of the library who want a deeper understanding of how the library works.***
