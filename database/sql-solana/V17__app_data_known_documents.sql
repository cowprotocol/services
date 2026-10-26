-- The swap document the trading SDK hashes into the intent per environment
-- without uploading it, so orders naming its hash resolve to it.
INSERT INTO solana.app_data (contract_app_data, full_app_data)
VALUES
    ('\xb07cdcc91b0a44366d5faca96af923b13f1844d74a69ee9ceebf1b982ee71b54',
     convert_to('{"appCode":"CoW Swap","environment":"staging","metadata":{"orderClass":{"orderClass":"market"}}}', 'UTF8')),
    ('\xb422e6757329ae3a68e111f6a941c761e92210501d45b8e1a49da3b274c22d92',
     convert_to('{"appCode":"CoW Swap","environment":"prod","metadata":{"orderClass":{"orderClass":"market"}}}', 'UTF8'))
ON CONFLICT DO NOTHING;
