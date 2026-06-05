: quit
  \ TODO: Exit return stack, set source-id, and set input device to user input.
  postpone [
  begin
    refill
  while
    (interpret)
  repeat
;
