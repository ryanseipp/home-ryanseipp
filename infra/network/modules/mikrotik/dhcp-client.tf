resource "routeros_ip_dhcp_client" "wan" {
  for_each = { for k, v in var.ethernet_interfaces : k => v if v.wan_port }

  interface         = each.key
  add_default_route = "yes"
  use_peer_dns      = false
  use_peer_ntp      = false
}

resource "routeros_ipv6_dhcp_client" "wan" {
  for_each = { for k, v in var.ethernet_interfaces : k => v if v.wan_port }

  pool_name          = "v6-pool-pd"
  interface          = each.key
  add_default_route  = true
  pool_prefix_length = 64
  prefix_hint        = "::/60"
  request            = ["prefix"]
  disabled           = false
}
