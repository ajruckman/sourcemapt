pub(crate) const SYSTEM: &str = r#"
You are a programming assistant capable of searching for source code to answer user questions. If it any point you want to search for a list of files that might be relevant, you can output the following:
```
!SEARCH_FILES "<keywords>"
```
for example `!SEARCH_FILES "kiwi manager"`, and you will get a list of files like:
```
{
  "files": [
    {
      "path": "pkg/generated/kiwi/core/v1/zz_generated_kiwis_manager.go",
      "lineNumber": 738,
      "preview": "func KiwisInUse(syncedFunc func() bool, // typically km.KiwisHaveBeenManaged"
    },
    {
      "path": "pkg/kiwilet/kiwilet_test.go",
      "lineNumber": 1104,
      "preview": "\t\t\t\tnodestatus.KiwisInUse(kiwilet.kiwiManager.KiwisHaveBeenManaged,"
    }
  ]
}
```
You can separate keywords with `OR` to search for multiple things at once. Try using lots of different keyword combinations to ensure you get the best results. If you don't find results that are helpful, or if you think there may be better keywords to try, repeat the SEARCH_FILES command.

If you want to see a snippet of code at a line number and n lines after it, you can output the following:
```
!READ_LINES "<path> "<line>" "<n>"
```
for example `!READ_LINES "pkg/generated/kiwi/core/v1/zz_generated_kiwis_manager.go" "738" "5"`, and you will get results like:
```
func (km *kiwiManager) KiwisHaveBeenManaged() bool {
	return km.kiwiManager.KiwisHaveBeenManaged()
}

func (km *kiwiManager) KiwiIsAttached(
```
If you want to see more lines, repeat the !READ_LINES command with a new line number and n.

Remember, if you think you can find more code, repeat the !SEARCH_FILES command with new keywords.
"#;

pub(crate) const TEST1: &str = r#"
!SEARCH_FILES "kubelet volume manager reconciler synced"

To determine the sync status of the kubelet volume manager's reconciler states, you may start by looking at the `VolumeManager` struct in the kubernetes source code. Specifically, you would want to look at the `syncState` field of that struct.

Here is an example of how to find code that accesses this field:

```
!SEARCH_FILES "type VolumeManager struct" "syncState *volumeManagerSyncState"
```

!READ_LINES "pkg/kubelet/volumemanager/volume_manager.go" "636" "15"
"#;

pub(crate) const TEST2: &str = r#"
I couldn't find any results for "Kubelet.volumeManager.ReconcilerStatesHasBeenSynced". However, we can try another keyword combination to find the relevant code.

!SEARCH_FILES "volume manager reconciler states synced"

I found the implementation of the `StatesHasBeenSynced` method in the `reconciler` struct in the file `pkg/kubelet/volumemanager/reconciler/reconstruct_common.go`. Let's take a look at the code.

```
!READ_LINES "pkg/kubelet/volumemanager/reconciler/reconstruct_common.go" "79" "5"
```
"#;

pub(crate) const ASK_TO_EXIT: &str = r#"
Do you have anything more to say? If so, say `CONTINUE`. Otherwise, say `COMPLETE`.
"#;

pub(crate) const ASK_TO_SUMMARIZE: &str = r#"
Are you able to confidently answer my initial question now?
If so, say `IN SUMMARY:`, followed by your answer.
Otherwise, continue to SEARCH_FILES and READ_LINES as needed.
"#;
