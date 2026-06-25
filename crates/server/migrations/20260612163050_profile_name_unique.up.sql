-- Profile names are unique per account (item 0012 / ADR-0009): a switcher needs unambiguous
-- labels within one account. Uniqueness is per `user_id`, so cross-account name collisions
-- stay allowed; the handler maps the violation to `409 profile_name_taken`. The register
-- path only ever creates one name per account, so this applies cleanly to existing data.
ALTER TABLE profiles ADD CONSTRAINT profiles_user_id_name_key UNIQUE (user_id, name);
