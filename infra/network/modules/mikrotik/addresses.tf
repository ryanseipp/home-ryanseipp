# =================================================================================================
# IPv4 Addresses
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/ip_address
# =================================================================================================
resource "routeros_ip_address" "loopback" {
  address   = "${local.node.loopback_ip4}/32"
  interface = "lo"
}

resource "routeros_ip_address" "peers" {
  for_each = local.node.peers

  address   = "${cidrhost(each.value.cidrs.v4, local.node.mode == "spine" ? 0 : 1)}/${split("/", each.value.cidrs.v4)[1]}"
  interface = each.value.iface
  comment   = "Peer to ${each.key}"
}


# =================================================================================================
# IPv6 Addresses
# https://registry.terraform.io/providers/terraform-routeros/routeros/latest/docs/resources/ipv6_address
# =================================================================================================
resource "routeros_ipv6_address" "loopback" {
  address   = "${local.node.loopback_ip6}/128"
  interface = "lo"
}

resource "routeros_ipv6_address" "peers" {
  for_each = local.node.peers

  address   = "${cidrhost(each.value.cidrs.v6, local.node.mode == "spine" ? 0 : 1)}/${split("/", each.value.cidrs.v6)[1]}"
  interface = each.value.iface
  comment   = "Peer to ${each.key}"
}
