# Brush

An experimental, GPU driven, and heavily opinionated painting program built on Rust, GTK and Libadwaita.

The project is currently under heavy development and is most certainly not ready
for any kind of serious usage.

It won't break anything, it's just kind of useless and unfinished as you'd expect.

Heavily inspired by [Krita](https://krita.org/), as is what I use day to day for my art.

## Notice of LLM Usage
Part of the code has been developed with the help of the Gemini LLM in a
non-integrated manner to aid me in the learning process of these graphics.
(In other words, I asked the Gemini chat website to explain things and give me snippets)

This affects parts of the OpenGL ES renderer as this is my first real go at
working with graphics programming.

The code does and will eventually get completely replaced with human written work as I make my way around learning the concepts and intricacies of graphics programming.

## TODO
- [x] Saving and loading
- [x] Alpha locking
- [ ] Exporting
- [ ] Brush engine
    - [x] Paint dab blending
    - [x] Paint stroke interpolation
    - [ ] Paint stroke smoothing/stabilizing
    - [ ] Brush types
- [ ] Settings and resource handling
    - [ ] Recently opened projects / Editor state
    - [ ] Preferences
    - [ ] Resources
- [ ] Tools
    - [ ] Fill
    - [ ] Line
    - [ ] Rectangle
    - [ ] Ellipse
    - [ ] Select box
    - [ ] Select ellipse
    - [ ] Select wand
    - [ ] Transform
- [ ] Canvas interaction
    - [x] Middle-click / space to pan
    - [ ] Up/down layer traversal hotkeys
    - [ ] Mirror canvas toggles
    - [ ] Layer dragging on the tree widget
    - [ ] Undo/redo
    - [ ] Layer opacity spin button
