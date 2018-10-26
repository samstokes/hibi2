CREATE VIEW user_with_bigid AS
  SELECT *, id::BIGINT AS bigid
  FROM "user"
  ;
CREATE VIEW ext_task_with_bigid AS
  SELECT *, id::BIGINT AS bigid
  FROM ext_task
  ;
