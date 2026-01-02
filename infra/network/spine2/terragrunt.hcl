locals {
  hostname      = "spine2"
  shared_locals = read_terragrunt_config(find_in_parent_folders("locals.hcl")).locals
}

terraform {
  source = find_in_parent_folders("modules/mikrotik")
}

inputs = {
  mikrotik_hostname = "https://${local.shared_locals.nodes[local.hostname].management_ip4}"
  mikrotik_username = get_env("MIKROTIK_USERNAME")
  mikrotik_password = get_env("MIKROTIK_PASSWORD")
  mikrotik_insecure = true

  certificate_common_name = local.shared_locals.nodes[local.hostname].management_ip4
  hostname                = local.hostname
  nodes                   = local.shared_locals.nodes
  ntp_servers             = [local.shared_locals.ntp_server]
  timezone                = local.shared_locals.timezone
}
