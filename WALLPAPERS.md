# wallpapers/ — provenance

**Not covered by the repository's LICENSE.** The MIT licence at the repo root
covers the code and configuration in this repository. It does not, and cannot,
cover these images: they were collected rather than authored here, and their
provenance was never recorded.

What is actually known, from the files themselves:

- 106 images, ~286 MB — the large majority of this repository by size.
- Two carry creator metadata that survived: `abstract-lock.png` names an
  individual artist in its XMP `Author` field, and `tanjiro-kamado-gruv.jpg`
  carries creator metadata for art derived from a copyrighted series.
- Six are named `gruv-wallhaven-*`, i.e. sourced from wallhaven.cc, where
  individual uploads carry their own separate terms.
- The remaining 104 carry no author or copyright metadata at all — stripped,
  or never present. Absence of a marker is not evidence of permission.

So: redistributing this directory has not been cleared, and `dotctl deploy`
copies every file in it onto the disk of anyone who installs these dotfiles.

This file lives at the repo root rather than inside `wallpapers/`, because
`dotctl deploy` copies every file in that directory into
`~/Pictures/Wallpapers`, where noctalia's picker would list it as an image.

This file records the position honestly rather than resolving it. Resolving it
means one of:

- attributing each image to its source and honouring each licence,
- keeping only images whose licence permits redistribution,
- or dropping the directory from the repository and fetching wallpapers at
  deploy time from wherever they legitimately live.

Until then, treat these as "present but unlicensed", not as part of the MIT
grant above.
