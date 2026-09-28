/// <reference types="@raycast/api">

/* 🚧 🚧 🚧
 * This file is auto-generated from the extension's manifest.
 * Do not modify manually. Instead, update the `package.json` file.
 * 🚧 🚧 🚧 */

/* eslint-disable @typescript-eslint/ban-types */

type ExtensionPreferences = {
  /** wallbar CLI - Path to the wallbar binary. */
  "cliPath": string
}

/** Preferences accessible in all the extension's commands */
declare type Preferences = ExtensionPreferences

declare namespace Preferences {
  /** Preferences accessible in the `about` command */
  export type About = ExtensionPreferences & {}
  /** Preferences accessible in the `shuffle` command */
  export type Shuffle = ExtensionPreferences & {}
  /** Preferences accessible in the `next` command */
  export type Next = ExtensionPreferences & {}
  /** Preferences accessible in the `previous` command */
  export type Previous = ExtensionPreferences & {}
  /** Preferences accessible in the `browse` command */
  export type Browse = ExtensionPreferences & {}
}

declare namespace Arguments {
  /** Arguments passed to the `about` command */
  export type About = {}
  /** Arguments passed to the `shuffle` command */
  export type Shuffle = {}
  /** Arguments passed to the `next` command */
  export type Next = {}
  /** Arguments passed to the `previous` command */
  export type Previous = {}
  /** Arguments passed to the `browse` command */
  export type Browse = {}
}

