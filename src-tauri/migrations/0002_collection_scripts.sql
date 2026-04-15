ALTER TABLE collections
  ADD COLUMN prerequest_script TEXT NOT NULL DEFAULT '';

ALTER TABLE collections
  ADD COLUMN test_script TEXT NOT NULL DEFAULT '';
