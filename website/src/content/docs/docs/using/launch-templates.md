---
title: Launch templates
description: Save launcher choices under a name, and apply them when you start a session.
sidebar:
  order: 2
  badge:
    text: Stub
    variant: caution
---

:::note[Stub]

This page is planned but not written yet. The text below says what it will cover.

:::

What a launch template is: a named set of launcher choices, any of which it may leave out. How to create, edit, and
delete templates in the **templates** panel beside **new**, and how to apply one in the
[session launcher](/docs/using/start-a-session/) by typing `tl:` and its name, which makes the same choices you could
make by hand. Applying several in turn, with the later one winning where they overlap. Why a template with a choice that
does not fit is refused as a whole. Why editing or deleting a template never changes a session started from it. And how
agents use templates with `farhelm agent create --template` and `farhelm spawn --template`, which apply them the same
way but cannot create or change them.
