import path from 'node:path';

// Every path a plugin reads is anchored here, not at the process working
// directory: an editor runs ESLint from wherever the file is.
export const landingRoot = path.dirname(import.meta.dirname);

// An import either sits beside the file or is named. Walking up out of a
// directory says nothing about what is being reached for.
export const upADirectory = {
  group: ['..', '../*', '../**'],
  message: 'Import from ./ or by alias (@app/, @brand/), never up a directory.',
};

// A union is of named types, never of shapes written into it, so it reads as
// a list of names.
export const namedUnionMembers = {
  selector: 'TSUnionType > TSTypeLiteral',
  message: 'Name each member of a union as a type of its own, then join the names.',
};
