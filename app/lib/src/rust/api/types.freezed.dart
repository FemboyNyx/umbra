// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'types.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$CoreEvent {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'CoreEvent()';
}


}

/// @nodoc
class $CoreEventCopyWith<$Res>  {
$CoreEventCopyWith(CoreEvent _, $Res Function(CoreEvent) __);
}


/// Adds pattern-matching-related methods to [CoreEvent].
extension CoreEventPatterns on CoreEvent {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( CoreEvent_MessageReceived value)?  messageReceived,TResult Function( CoreEvent_MessageStatus value)?  messageStatus,TResult Function( CoreEvent_ContactOnline value)?  contactOnline,TResult Function( CoreEvent_ContactOffline value)?  contactOffline,TResult Function( CoreEvent_ContactRequest value)?  contactRequest,TResult Function( CoreEvent_Signal value)?  signal,TResult Function( CoreEvent_NetworkStatus value)?  networkStatus,required TResult orElse(),}){
final _that = this;
switch (_that) {
case CoreEvent_MessageReceived() when messageReceived != null:
return messageReceived(_that);case CoreEvent_MessageStatus() when messageStatus != null:
return messageStatus(_that);case CoreEvent_ContactOnline() when contactOnline != null:
return contactOnline(_that);case CoreEvent_ContactOffline() when contactOffline != null:
return contactOffline(_that);case CoreEvent_ContactRequest() when contactRequest != null:
return contactRequest(_that);case CoreEvent_Signal() when signal != null:
return signal(_that);case CoreEvent_NetworkStatus() when networkStatus != null:
return networkStatus(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( CoreEvent_MessageReceived value)  messageReceived,required TResult Function( CoreEvent_MessageStatus value)  messageStatus,required TResult Function( CoreEvent_ContactOnline value)  contactOnline,required TResult Function( CoreEvent_ContactOffline value)  contactOffline,required TResult Function( CoreEvent_ContactRequest value)  contactRequest,required TResult Function( CoreEvent_Signal value)  signal,required TResult Function( CoreEvent_NetworkStatus value)  networkStatus,}){
final _that = this;
switch (_that) {
case CoreEvent_MessageReceived():
return messageReceived(_that);case CoreEvent_MessageStatus():
return messageStatus(_that);case CoreEvent_ContactOnline():
return contactOnline(_that);case CoreEvent_ContactOffline():
return contactOffline(_that);case CoreEvent_ContactRequest():
return contactRequest(_that);case CoreEvent_Signal():
return signal(_that);case CoreEvent_NetworkStatus():
return networkStatus(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( CoreEvent_MessageReceived value)?  messageReceived,TResult? Function( CoreEvent_MessageStatus value)?  messageStatus,TResult? Function( CoreEvent_ContactOnline value)?  contactOnline,TResult? Function( CoreEvent_ContactOffline value)?  contactOffline,TResult? Function( CoreEvent_ContactRequest value)?  contactRequest,TResult? Function( CoreEvent_Signal value)?  signal,TResult? Function( CoreEvent_NetworkStatus value)?  networkStatus,}){
final _that = this;
switch (_that) {
case CoreEvent_MessageReceived() when messageReceived != null:
return messageReceived(_that);case CoreEvent_MessageStatus() when messageStatus != null:
return messageStatus(_that);case CoreEvent_ContactOnline() when contactOnline != null:
return contactOnline(_that);case CoreEvent_ContactOffline() when contactOffline != null:
return contactOffline(_that);case CoreEvent_ContactRequest() when contactRequest != null:
return contactRequest(_that);case CoreEvent_Signal() when signal != null:
return signal(_that);case CoreEvent_NetworkStatus() when networkStatus != null:
return networkStatus(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( Message message)?  messageReceived,TResult Function( String id,  String peerId,  DeliveryStatus status)?  messageStatus,TResult Function( String peerId)?  contactOnline,TResult Function( String peerId)?  contactOffline,TResult Function( ContactCard card)?  contactRequest,TResult Function( String peerId,  String callId,  SignalKind kind,  String payload)?  signal,TResult Function( NetworkInfo info)?  networkStatus,required TResult orElse(),}) {final _that = this;
switch (_that) {
case CoreEvent_MessageReceived() when messageReceived != null:
return messageReceived(_that.message);case CoreEvent_MessageStatus() when messageStatus != null:
return messageStatus(_that.id,_that.peerId,_that.status);case CoreEvent_ContactOnline() when contactOnline != null:
return contactOnline(_that.peerId);case CoreEvent_ContactOffline() when contactOffline != null:
return contactOffline(_that.peerId);case CoreEvent_ContactRequest() when contactRequest != null:
return contactRequest(_that.card);case CoreEvent_Signal() when signal != null:
return signal(_that.peerId,_that.callId,_that.kind,_that.payload);case CoreEvent_NetworkStatus() when networkStatus != null:
return networkStatus(_that.info);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( Message message)  messageReceived,required TResult Function( String id,  String peerId,  DeliveryStatus status)  messageStatus,required TResult Function( String peerId)  contactOnline,required TResult Function( String peerId)  contactOffline,required TResult Function( ContactCard card)  contactRequest,required TResult Function( String peerId,  String callId,  SignalKind kind,  String payload)  signal,required TResult Function( NetworkInfo info)  networkStatus,}) {final _that = this;
switch (_that) {
case CoreEvent_MessageReceived():
return messageReceived(_that.message);case CoreEvent_MessageStatus():
return messageStatus(_that.id,_that.peerId,_that.status);case CoreEvent_ContactOnline():
return contactOnline(_that.peerId);case CoreEvent_ContactOffline():
return contactOffline(_that.peerId);case CoreEvent_ContactRequest():
return contactRequest(_that.card);case CoreEvent_Signal():
return signal(_that.peerId,_that.callId,_that.kind,_that.payload);case CoreEvent_NetworkStatus():
return networkStatus(_that.info);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( Message message)?  messageReceived,TResult? Function( String id,  String peerId,  DeliveryStatus status)?  messageStatus,TResult? Function( String peerId)?  contactOnline,TResult? Function( String peerId)?  contactOffline,TResult? Function( ContactCard card)?  contactRequest,TResult? Function( String peerId,  String callId,  SignalKind kind,  String payload)?  signal,TResult? Function( NetworkInfo info)?  networkStatus,}) {final _that = this;
switch (_that) {
case CoreEvent_MessageReceived() when messageReceived != null:
return messageReceived(_that.message);case CoreEvent_MessageStatus() when messageStatus != null:
return messageStatus(_that.id,_that.peerId,_that.status);case CoreEvent_ContactOnline() when contactOnline != null:
return contactOnline(_that.peerId);case CoreEvent_ContactOffline() when contactOffline != null:
return contactOffline(_that.peerId);case CoreEvent_ContactRequest() when contactRequest != null:
return contactRequest(_that.card);case CoreEvent_Signal() when signal != null:
return signal(_that.peerId,_that.callId,_that.kind,_that.payload);case CoreEvent_NetworkStatus() when networkStatus != null:
return networkStatus(_that.info);case _:
  return null;

}
}

}

/// @nodoc


class CoreEvent_MessageReceived extends CoreEvent {
  const CoreEvent_MessageReceived({required this.message}): super._();
  

 final  Message message;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CoreEvent_MessageReceivedCopyWith<CoreEvent_MessageReceived> get copyWith => _$CoreEvent_MessageReceivedCopyWithImpl<CoreEvent_MessageReceived>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent_MessageReceived&&(identical(other.message, message) || other.message == message));
}


@override
int get hashCode => Object.hash(runtimeType,message);

@override
String toString() {
  return 'CoreEvent.messageReceived(message: $message)';
}


}

/// @nodoc
abstract mixin class $CoreEvent_MessageReceivedCopyWith<$Res> implements $CoreEventCopyWith<$Res> {
  factory $CoreEvent_MessageReceivedCopyWith(CoreEvent_MessageReceived value, $Res Function(CoreEvent_MessageReceived) _then) = _$CoreEvent_MessageReceivedCopyWithImpl;
@useResult
$Res call({
 Message message
});




}
/// @nodoc
class _$CoreEvent_MessageReceivedCopyWithImpl<$Res>
    implements $CoreEvent_MessageReceivedCopyWith<$Res> {
  _$CoreEvent_MessageReceivedCopyWithImpl(this._self, this._then);

  final CoreEvent_MessageReceived _self;
  final $Res Function(CoreEvent_MessageReceived) _then;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? message = null,}) {
  return _then(CoreEvent_MessageReceived(
message: null == message ? _self.message : message // ignore: cast_nullable_to_non_nullable
as Message,
  ));
}


}

/// @nodoc


class CoreEvent_MessageStatus extends CoreEvent {
  const CoreEvent_MessageStatus({required this.id, required this.peerId, required this.status}): super._();
  

 final  String id;
 final  String peerId;
 final  DeliveryStatus status;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CoreEvent_MessageStatusCopyWith<CoreEvent_MessageStatus> get copyWith => _$CoreEvent_MessageStatusCopyWithImpl<CoreEvent_MessageStatus>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent_MessageStatus&&(identical(other.id, id) || other.id == id)&&(identical(other.peerId, peerId) || other.peerId == peerId)&&(identical(other.status, status) || other.status == status));
}


@override
int get hashCode => Object.hash(runtimeType,id,peerId,status);

@override
String toString() {
  return 'CoreEvent.messageStatus(id: $id, peerId: $peerId, status: $status)';
}


}

/// @nodoc
abstract mixin class $CoreEvent_MessageStatusCopyWith<$Res> implements $CoreEventCopyWith<$Res> {
  factory $CoreEvent_MessageStatusCopyWith(CoreEvent_MessageStatus value, $Res Function(CoreEvent_MessageStatus) _then) = _$CoreEvent_MessageStatusCopyWithImpl;
@useResult
$Res call({
 String id, String peerId, DeliveryStatus status
});




}
/// @nodoc
class _$CoreEvent_MessageStatusCopyWithImpl<$Res>
    implements $CoreEvent_MessageStatusCopyWith<$Res> {
  _$CoreEvent_MessageStatusCopyWithImpl(this._self, this._then);

  final CoreEvent_MessageStatus _self;
  final $Res Function(CoreEvent_MessageStatus) _then;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? id = null,Object? peerId = null,Object? status = null,}) {
  return _then(CoreEvent_MessageStatus(
id: null == id ? _self.id : id // ignore: cast_nullable_to_non_nullable
as String,peerId: null == peerId ? _self.peerId : peerId // ignore: cast_nullable_to_non_nullable
as String,status: null == status ? _self.status : status // ignore: cast_nullable_to_non_nullable
as DeliveryStatus,
  ));
}


}

/// @nodoc


class CoreEvent_ContactOnline extends CoreEvent {
  const CoreEvent_ContactOnline({required this.peerId}): super._();
  

 final  String peerId;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CoreEvent_ContactOnlineCopyWith<CoreEvent_ContactOnline> get copyWith => _$CoreEvent_ContactOnlineCopyWithImpl<CoreEvent_ContactOnline>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent_ContactOnline&&(identical(other.peerId, peerId) || other.peerId == peerId));
}


@override
int get hashCode => Object.hash(runtimeType,peerId);

@override
String toString() {
  return 'CoreEvent.contactOnline(peerId: $peerId)';
}


}

/// @nodoc
abstract mixin class $CoreEvent_ContactOnlineCopyWith<$Res> implements $CoreEventCopyWith<$Res> {
  factory $CoreEvent_ContactOnlineCopyWith(CoreEvent_ContactOnline value, $Res Function(CoreEvent_ContactOnline) _then) = _$CoreEvent_ContactOnlineCopyWithImpl;
@useResult
$Res call({
 String peerId
});




}
/// @nodoc
class _$CoreEvent_ContactOnlineCopyWithImpl<$Res>
    implements $CoreEvent_ContactOnlineCopyWith<$Res> {
  _$CoreEvent_ContactOnlineCopyWithImpl(this._self, this._then);

  final CoreEvent_ContactOnline _self;
  final $Res Function(CoreEvent_ContactOnline) _then;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? peerId = null,}) {
  return _then(CoreEvent_ContactOnline(
peerId: null == peerId ? _self.peerId : peerId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class CoreEvent_ContactOffline extends CoreEvent {
  const CoreEvent_ContactOffline({required this.peerId}): super._();
  

 final  String peerId;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CoreEvent_ContactOfflineCopyWith<CoreEvent_ContactOffline> get copyWith => _$CoreEvent_ContactOfflineCopyWithImpl<CoreEvent_ContactOffline>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent_ContactOffline&&(identical(other.peerId, peerId) || other.peerId == peerId));
}


@override
int get hashCode => Object.hash(runtimeType,peerId);

@override
String toString() {
  return 'CoreEvent.contactOffline(peerId: $peerId)';
}


}

/// @nodoc
abstract mixin class $CoreEvent_ContactOfflineCopyWith<$Res> implements $CoreEventCopyWith<$Res> {
  factory $CoreEvent_ContactOfflineCopyWith(CoreEvent_ContactOffline value, $Res Function(CoreEvent_ContactOffline) _then) = _$CoreEvent_ContactOfflineCopyWithImpl;
@useResult
$Res call({
 String peerId
});




}
/// @nodoc
class _$CoreEvent_ContactOfflineCopyWithImpl<$Res>
    implements $CoreEvent_ContactOfflineCopyWith<$Res> {
  _$CoreEvent_ContactOfflineCopyWithImpl(this._self, this._then);

  final CoreEvent_ContactOffline _self;
  final $Res Function(CoreEvent_ContactOffline) _then;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? peerId = null,}) {
  return _then(CoreEvent_ContactOffline(
peerId: null == peerId ? _self.peerId : peerId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class CoreEvent_ContactRequest extends CoreEvent {
  const CoreEvent_ContactRequest({required this.card}): super._();
  

 final  ContactCard card;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CoreEvent_ContactRequestCopyWith<CoreEvent_ContactRequest> get copyWith => _$CoreEvent_ContactRequestCopyWithImpl<CoreEvent_ContactRequest>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent_ContactRequest&&(identical(other.card, card) || other.card == card));
}


@override
int get hashCode => Object.hash(runtimeType,card);

@override
String toString() {
  return 'CoreEvent.contactRequest(card: $card)';
}


}

/// @nodoc
abstract mixin class $CoreEvent_ContactRequestCopyWith<$Res> implements $CoreEventCopyWith<$Res> {
  factory $CoreEvent_ContactRequestCopyWith(CoreEvent_ContactRequest value, $Res Function(CoreEvent_ContactRequest) _then) = _$CoreEvent_ContactRequestCopyWithImpl;
@useResult
$Res call({
 ContactCard card
});




}
/// @nodoc
class _$CoreEvent_ContactRequestCopyWithImpl<$Res>
    implements $CoreEvent_ContactRequestCopyWith<$Res> {
  _$CoreEvent_ContactRequestCopyWithImpl(this._self, this._then);

  final CoreEvent_ContactRequest _self;
  final $Res Function(CoreEvent_ContactRequest) _then;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? card = null,}) {
  return _then(CoreEvent_ContactRequest(
card: null == card ? _self.card : card // ignore: cast_nullable_to_non_nullable
as ContactCard,
  ));
}


}

/// @nodoc


class CoreEvent_Signal extends CoreEvent {
  const CoreEvent_Signal({required this.peerId, required this.callId, required this.kind, required this.payload}): super._();
  

 final  String peerId;
 final  String callId;
 final  SignalKind kind;
 final  String payload;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CoreEvent_SignalCopyWith<CoreEvent_Signal> get copyWith => _$CoreEvent_SignalCopyWithImpl<CoreEvent_Signal>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent_Signal&&(identical(other.peerId, peerId) || other.peerId == peerId)&&(identical(other.callId, callId) || other.callId == callId)&&(identical(other.kind, kind) || other.kind == kind)&&(identical(other.payload, payload) || other.payload == payload));
}


@override
int get hashCode => Object.hash(runtimeType,peerId,callId,kind,payload);

@override
String toString() {
  return 'CoreEvent.signal(peerId: $peerId, callId: $callId, kind: $kind, payload: $payload)';
}


}

/// @nodoc
abstract mixin class $CoreEvent_SignalCopyWith<$Res> implements $CoreEventCopyWith<$Res> {
  factory $CoreEvent_SignalCopyWith(CoreEvent_Signal value, $Res Function(CoreEvent_Signal) _then) = _$CoreEvent_SignalCopyWithImpl;
@useResult
$Res call({
 String peerId, String callId, SignalKind kind, String payload
});




}
/// @nodoc
class _$CoreEvent_SignalCopyWithImpl<$Res>
    implements $CoreEvent_SignalCopyWith<$Res> {
  _$CoreEvent_SignalCopyWithImpl(this._self, this._then);

  final CoreEvent_Signal _self;
  final $Res Function(CoreEvent_Signal) _then;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? peerId = null,Object? callId = null,Object? kind = null,Object? payload = null,}) {
  return _then(CoreEvent_Signal(
peerId: null == peerId ? _self.peerId : peerId // ignore: cast_nullable_to_non_nullable
as String,callId: null == callId ? _self.callId : callId // ignore: cast_nullable_to_non_nullable
as String,kind: null == kind ? _self.kind : kind // ignore: cast_nullable_to_non_nullable
as SignalKind,payload: null == payload ? _self.payload : payload // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class CoreEvent_NetworkStatus extends CoreEvent {
  const CoreEvent_NetworkStatus({required this.info}): super._();
  

 final  NetworkInfo info;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CoreEvent_NetworkStatusCopyWith<CoreEvent_NetworkStatus> get copyWith => _$CoreEvent_NetworkStatusCopyWithImpl<CoreEvent_NetworkStatus>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CoreEvent_NetworkStatus&&(identical(other.info, info) || other.info == info));
}


@override
int get hashCode => Object.hash(runtimeType,info);

@override
String toString() {
  return 'CoreEvent.networkStatus(info: $info)';
}


}

/// @nodoc
abstract mixin class $CoreEvent_NetworkStatusCopyWith<$Res> implements $CoreEventCopyWith<$Res> {
  factory $CoreEvent_NetworkStatusCopyWith(CoreEvent_NetworkStatus value, $Res Function(CoreEvent_NetworkStatus) _then) = _$CoreEvent_NetworkStatusCopyWithImpl;
@useResult
$Res call({
 NetworkInfo info
});




}
/// @nodoc
class _$CoreEvent_NetworkStatusCopyWithImpl<$Res>
    implements $CoreEvent_NetworkStatusCopyWith<$Res> {
  _$CoreEvent_NetworkStatusCopyWithImpl(this._self, this._then);

  final CoreEvent_NetworkStatus _self;
  final $Res Function(CoreEvent_NetworkStatus) _then;

/// Create a copy of CoreEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? info = null,}) {
  return _then(CoreEvent_NetworkStatus(
info: null == info ? _self.info : info // ignore: cast_nullable_to_non_nullable
as NetworkInfo,
  ));
}


}

// dart format on
