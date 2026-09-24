#!/usr/bin/env python3
import pathlib
import posixpath
import sys
import tarfile

archive, destination = sys.argv[1:]
with tarfile.open(archive, "r:gz") as stream:
    members = stream.getmembers()
    if not members:
        raise SystemExit("published Vespa base archive is empty")

    by_name = {str(pathlib.PurePosixPath(member.name)): member for member in members}
    symlinks = set()
    for member in members:
        path = pathlib.PurePosixPath(member.name)
        if path.is_absolute() or ".." in path.parts:
            raise SystemExit(f"unsafe published Vespa base path: {member.name}")
        if member.isdev():
            raise SystemExit(f"unsupported published Vespa base entry: {member.name}")
        if member.issym():
            target = pathlib.PurePosixPath(member.linkname)
            if target.is_absolute():
                raise SystemExit(f"unsafe published Vespa base symlink: {member.name}")
            resolved = posixpath.normpath(str(path.parent / target))
            if resolved == ".." or resolved.startswith("../"):
                raise SystemExit(f"unsafe published Vespa base symlink: {member.name}")
            symlinks.add(path)
        elif member.islnk():
            target = pathlib.PurePosixPath(member.linkname)
            if target.is_absolute() or ".." in target.parts:
                raise SystemExit(f"unsafe published Vespa base hardlink: {member.name}")
            resolved = posixpath.normpath(str(target))
            target_member = by_name.get(resolved)
            if target_member is None or not target_member.isfile():
                raise SystemExit(f"unsafe published Vespa base hardlink: {member.name}")

    # A later archive member must never be extracted through a symlink created
    # by an earlier member. Legitimate runtime SONAME links point at sibling
    # files and have no children of their own.
    for member in members:
        path = pathlib.PurePosixPath(member.name)
        for parent in path.parents:
            if parent in symlinks:
                raise SystemExit(f"unsafe published Vespa base symlink traversal: {member.name}")

    stream.extractall(destination, members=members)
