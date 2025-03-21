<div align="center">
  <pre>
^    ^
^  ^
( )
kestrel
  </pre>
</div>

# kestrel

Kestrel is a framework for process orchestration combining imperative and declarative programming paradigms. It is intended for rapid prototyping of highly interdependent systems, particularly in the context of distributed computing and networking.

## Getting started
We're working on getting this into a user-ready state. Check back soon!

## Contributing

| Task | Description |
|------|-------------|
| [Upcoming Events](https://github.com/movementlabsxyz/ffs/issues?q=is%3Aissue%20state%3Aopen%20label%3Apriority%3Ahigh%2Cpriority%3Amedium%20label%3Aevent) | High-priority `event` issues with planned completion dates. |
| [Release Candidates](https://github.com/movementlabsxyz/ffs/issues?q=is%3Aissue%20state%3Aopen%20label%3Arelease-candidate) | Feature-complete versions linked to events. |
| [Features & Bugs](https://github.com/movementlabsxyz/ffs/issues?q=is%3Aissue%20state%3Aopen%20label%3Afeature%2Cbug%20label%3Apriority%3Aurgent%2Cpriority%3Ahigh) | High-priority `feature` and `bug` issues. |

Please see the [CONTRIBUTING.md](CONTRIBUTING.md) file for contribution guidelines.

## Organization
There are five subdirectories which progressively build on one another for node logic.

1. [`util`](./util): contains utility logic mainly reused in [`protocol`](./protocol).
2. [`kestrel`](./kestrel): contains core kestrel crates including `kestrel`
