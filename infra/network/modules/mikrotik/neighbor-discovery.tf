# ================================================================================================
# IPv6 Neighbor Discovery
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/ipv6_dhcp_server
# ================================================================================================
# resource "routeros_ipv6_neighbor_discovery" "this" {
#   for_each = var.vlans
#
#   name         = each.value.name
#   comment      = "${each.value.name} DHCP Server"
#   address_pool = "v6-pool-pd"
#   interface    = each.value.name
#
#   advertise_dns = true
#   dns_servers   = "temp"
#   pref64        = each.value.ipv6_only ? ["64:ff9b::/96"] : []
# }
