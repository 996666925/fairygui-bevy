# Loader content and load state

GLoader can load an exported component through either native `ui://` URL form,
in addition to image and movie content. `GuiRuntime::loader_component(loader)`
returns its current component ID. The runtime owns that component's lifetime;
applications can use the ID to customize its controls, controllers or transitions.

The component and its descendants are real core objects and Bevy views, with their
own source package handles. They are adopted into the loader's display subtree
before initial view creation. This model relationship supplies display ownership,
stage propagation, inherited opacity and input bubbling; it is an adapter
representation of native GLoader's directly attached display object. Native
GLoader does not set its content GObject's logical parent. Code inspecting core
parent pointers should account for this representation.

All six fill modes, horizontal/vertical alignment, shrink-only and auto-size use
the package item's source dimensions. Default component layout changes scale;
`set_loader_use_resize(loader, true)` changes the component's logical size instead,
allowing its own relations to reposition/resize children. As in native updateLayout,
switching modes does not reset a size/scale previously written by the other mode.
Auto-size uses source dimensions and native 50x30 fallbacks for zero dimensions.
Loader color/Playing/Frame continue to target image/movie content; they do not
recolor or control movies nested inside a loaded component.

Changing URL clears and disposes old component ownership, including its nested
Loader components. Image/movie/component/missing/empty URL transitions preserve
the Loader object/view. Hiding or detaching a retained loader keeps its content;
disposing its owner removes it. A missing dependency stays empty and is retried
when the package becomes available. Existing component content retains its source
handle. The implementation does not establish a general asset hot-reload policy.
Recursive component-loader references are rejected rather than growing the object
store across frames.

Component synchronization runs at the start of Sync, before roots/pending objects
spawn. It initializes only the newly loaded subtree's title/list/group/scrollbar
state. Newly introduced nested loaders are processed in the same pass. Geometry
updates run from outer to inner owners so parent resizing and nested relations
reach their final model state in that synchronization.

Loaded controls receive normal pointer routing, button actions and events. Their
transitions participate in ordinary stage entry/exit and advancement. Loading a
component creates new content rather than taking an application instance from a
pool, and clearing the URL disposes that content.

## Native evidence and validation

The reference is FairyGUI-layabox GLoader.ts loadFromPackage, updateLayout,
clearContent, component getter and disposal. The executable fixture
`crates/fairygui-core/tests/fixtures/loader-component-reference.txt` runs the
unchanged GLoader class from source/bin/fairygui.js under a minimal component host.
Its GLoader SHA256 is
`3e4bd17230cadb0db174603c4885bc9441effe52e67d2df82f1feac2d8dde162`.
Rust tests consume the checked-in reference trace directly.
The trace covers 42 cases and 70 layouts: fill modes, shrink-only, all alignments,
auto-size including zero source dimensions, resize versus scale and mode changes.
It compares component position, logical size, scale and loader dimensions; the
host does not implement native rendering, package loading or display events.

Bevy tests use official exported components to verify first-frame package/atlas
ownership, component relations, stable views on layout changes, URL replacement
cleanup, nested ownership, hidden/offstage retention, dependency retry, cycle
rejection, loaded-button input and transition disposal. Windows smoke extends
Basics/Loader with a real component, scale/resize relation changes, a loaded
button click and image/missing/empty/restored URL changes. Captures are
`target/fairygui-demo-loader-component-scale.png` and
`target/fairygui-demo-loader-component-resize.png`.

These checks establish component construction, layout, input and lifecycle
behavior. They do not establish native pixel parity or exact Laya scheduling.

## Load state, error signs and retry

`GuiRuntime::loader_load_state(loader)` returns the most recent Sync result:
`Empty`, `Loading`, `Ready`, or `Failed(GuiLoaderFailure)`. It returns `None`
before the loader's first synchronization or after its disposal is synchronized.
Failures distinguish missing package/item data, unsupported package resource
types, component construction failure and image load failure. Components become
ready when constructed; nested image failures do not turn their owner into a
failed component. Movie status checks the distinct atlases used by its frames.
Frames without a texture do not count as failed image requests.

Configure an exported error-sign resource using the native UIConfig equivalent:

```rust,ignore
runtime.set_loader_error_sign(Some("ui://MyPackage/LoaderError"));
runtime.store_mut().set_loader_show_error_sign(loader, true);
// After Sync, inspect or customize the currently displayed error object.
if let Some(sign) = runtime.loader_error_sign(loader) {
    runtime.store_mut().set_object_text(sign, "Resource unavailable");
}
// Reevaluate the current URL without replacing the Loader object/view.
runtime.retry_loader(loader);
```

No default error artwork is invented. An unset/missing/invalid template leaves
the failure queryable without showing a sign. Both native URL forms resolve to
the same pool key. Error signs use a dedicated FIFO pool which retains the model,
view and customized state; they are sized to the current loader dimensions when
acquired on failure. Resizing an existing failed loader does not resize its sign.
Changing `showErrorSign` or the configured resource alone neither adds nor removes
an existing sign. Those changes affect a subsequent failure, URL change or retry.
New core loaders default to `showErrorSign = true`; exported instances retain
their serialized flag.

Clearing/replacing the URL or recovering from failure detaches the sign and
returns it to the pool. Attached signs are owned by the loader subtree and are
destroyed if the loader is disposed. Pooled signs live until reused or the runtime
is dropped. Dead pool entries are skipped. Ancestor resource checks reject
recursive error-sign construction. As with component content, the adapter uses
a model parent for native direct display attachment; native GObject parent
inspection and disposal pooling are not claimed to be identical.

Image status observes Bevy's retained handle rather than calling `load` every
frame: in Bevy 0.20.1, `load` automatically restarts Failed requests. Explicit
`retry_loader` refreshes failed requests and rebuilds component/movie content.
The retry API is an adapter convenience. External URLs use the same package-relative
asset path resolution as existing loader visuals; this does not add an HTTP
fetcher. Missing packages can recover when their FuiAssets become available.

The error lifecycle fixture
`crates/fairygui-core/tests/fixtures/loader-error-reference.txt` records unchanged
GLoader and GObjectPool behavior with minimal package/display hosts. Its 30 steps compare
error size, show/config behavior, URL aliases, FIFO identity, retained title,
attachment and pool count. The hosts do not implement actual package loading or
native rendering. Additional Bevy tests cover actual asynchronous image failure,
retry, image/movie recovery, late packages, recursion and disposal. Windows smoke
captures `target/fairygui-demo-loader-error.png` and checks model/view reuse,
captured size, flag/retry semantics and successful component recovery.
